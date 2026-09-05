use super::protocol::{
    ReaderEvent, error_response, message_method, normalize_initialize_params, request_id,
    respond_to_stop_request, stop_request_id,
};
use super::{ClientSession, Daemon, INVALID_REQUEST, REQUEST_CANCELLED, SERVER_NOT_INITIALIZED};
use crate::error::Result;
use crate::lsp::transport::{log_debug_message, write_message};
use serde_json::Value;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

pub(super) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);
pub(super) const MAX_PENDING_CONNECTIONS: usize = 16;

pub(super) struct PendingConnection {
    pub(super) client: ClientSession,
    pub(super) deadline: Instant,
}

impl PendingConnection {
    fn reject(&mut self, id: &Value, code: i64, reason: &str, debug: bool) {
        let response = error_response(id, code, reason);
        log_debug_message(debug, "daemon pending client -> ", &response);
        // A rejected peer may already have closed; its response failure must not stop the daemon.
        let _ = write_message(&mut self.client.writer, &response);
    }
}

impl Daemon {
    pub(super) fn accept_pending_connection(
        &mut self,
        stream: UnixStream,
        accepted_at: Instant,
    ) -> Result<()> {
        let deadline = accepted_at + HANDSHAKE_TIMEOUT;
        if deadline <= Instant::now() || self.pending_connections.len() >= MAX_PENDING_CONNECTIONS {
            // Do not read a rejected newcomer: no request ID is known, and existing pending
            // clients retain their slots. This policy applies to control connections too.
            return Ok(());
        }
        let client = ClientSession::new(stream, &mut self.events, Some(deadline))?;
        self.pending_connections
            .insert(client.generation, PendingConnection { client, deadline });
        Ok(())
    }

    pub(super) fn expire_pending_connections(&mut self, now: Instant) {
        self.pending_connections
            .retain(|_, pending| pending.deadline > now);
    }

    pub(super) fn next_event_timeout(&self, now: Instant) -> Option<Duration> {
        let idle = if self.active_client.is_none() {
            Some(
                self.idle_timeout
                    .saturating_sub(now.saturating_duration_since(self.idle_since)),
            )
        } else {
            None
        };
        self.pending_connections
            .values()
            .map(|pending| pending.deadline.saturating_duration_since(now))
            .chain(idle)
            .min()
    }

    pub(super) fn handle_pending_message(
        &mut self,
        generation: u64,
        event: ReaderEvent,
    ) -> Result<()> {
        let Some(mut pending) = self.pending_connections.remove(&generation) else {
            return Ok(());
        };
        let ReaderEvent::Message(message) = event else {
            // Malformed frames and EOF belong to this unadmitted connection only.
            return Ok(());
        };
        log_debug_message(self.debug, "daemon pending client <- ", &message);
        if stop_request_id(&message).is_some() {
            if respond_to_stop_request(&mut pending.client.writer, &message, self.debug).is_ok() {
                self.stop_requested = true;
            }
            return Ok(());
        }
        let Some(id) = request_id(&message) else {
            return Ok(());
        };
        if message_method(&message) != Some("initialize") {
            pending.reject(
                &id,
                SERVER_NOT_INITIALIZED,
                "daemon client must initialize before sending requests",
                self.debug,
            );
            return Ok(());
        }
        if self.active_client.is_some() {
            pending.reject(
                &id,
                REQUEST_CANCELLED,
                "another daemon client is already connected",
                self.debug,
            );
            return Ok(());
        }
        let params = message.get("params").unwrap_or(&Value::Null);
        if let Err(error) = normalize_initialize_params(params, &self.target) {
            pending.reject(&id, INVALID_REQUEST, &error.to_string(), self.debug);
            return Ok(());
        }
        // Move the existing reader rather than reopening the stream: initialized and later
        // frames may already be buffered behind this first message in the same read.
        self.active_client = Some(pending.client);
        self.handle_client_message(&message)
    }
}

#[cfg(test)]
mod tests;
