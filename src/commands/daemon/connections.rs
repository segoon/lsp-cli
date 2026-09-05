use super::protocol::{
    ReaderEvent, error_response, message_method, normalize_initialize_params, request_id,
    stop_request_id, success_response,
};
use super::writer::WriteId;
use super::{ClientSession, Daemon, INVALID_REQUEST, REQUEST_CANCELLED, SERVER_NOT_INITIALIZED};
use crate::error::Result;
use serde_json::Value;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(super) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);
pub(super) const MAX_PENDING_CONNECTIONS: usize = 16;

pub(super) struct PendingConnection {
    pub(super) client: ClientSession,
    pub(super) deadline: Instant,
    pub(super) close_after_write: Option<WriteId>,
    pub(super) stop_after_write: bool,
}

impl PendingConnection {
    fn reject(&mut self, id: &Value, code: i64, reason: &str) -> Result<(WriteId, Value)> {
        let response = error_response(id, code, reason);
        self.client
            .writer
            .enqueue(&response)
            .map(|write_id| (write_id, response))
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
        self.pending_connections.insert(
            client.generation,
            PendingConnection {
                client,
                deadline,
                close_after_write: None,
                stop_after_write: false,
            },
        );
        Ok(())
    }

    pub(super) fn expire_pending_connections(&mut self, now: Instant) {
        self.pending_connections
            .retain(|_, pending| pending.close_after_write.is_some() || pending.deadline > now);
    }

    pub(super) fn next_event_timeout(&mut self, now: Instant) -> Option<Duration> {
        let idle = if self.active_client.is_none() {
            Some(
                self.idle_timeout
                    .saturating_sub(now.saturating_duration_since(self.idle_since)),
            )
        } else {
            None
        };
        for pending in self.pending_connections.values_mut() {
            pending.client.writer.refresh_flag(now);
        }
        if let Some(client) = self.active_client.as_mut() {
            client.writer.refresh_flag(now);
        }
        if let Some(upstream) = self.upstream.as_mut() {
            upstream.writer.refresh_flag(now);
        }
        let output_deadlines = self
            .pending_connections
            .values()
            .filter_map(|pending| pending.client.writer.deadline(self.write_stall_timeout))
            .chain(
                self.active_client
                    .as_ref()
                    .and_then(|client| client.writer.deadline(self.write_stall_timeout)),
            )
            .chain(
                self.upstream
                    .as_ref()
                    .and_then(|upstream| upstream.writer.deadline(self.write_stall_timeout)),
            );
        self.pending_connections
            .values()
            .filter(|pending| pending.close_after_write.is_none())
            .map(|pending| pending.deadline.saturating_duration_since(now))
            .chain(idle)
            .chain(output_deadlines.map(|deadline| deadline.saturating_duration_since(now)))
            .chain(self.lifecycle_timeout(now))
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
        self.logger
            .debug("daemon pending client <- ", Arc::clone(&message));
        if let Some(id) = stop_request_id(&message) {
            let response = success_response(&id, &Value::Null);
            self.logger
                .debug_value("daemon pending client -> ", &response);
            if let Ok(write_id) = pending.client.writer.enqueue(&response) {
                pending.close_after_write = Some(write_id);
                pending.stop_after_write = true;
                self.pending_connections.insert(generation, pending);
            }
            return Ok(());
        }
        let Some(id) = request_id(&message) else {
            return Ok(());
        };
        if message_method(&message) != Some("initialize") {
            if let Ok((write_id, response)) = pending.reject(
                &id,
                SERVER_NOT_INITIALIZED,
                "daemon client must initialize before sending requests",
            ) {
                self.logger
                    .debug_value("daemon pending client -> ", &response);
                pending.close_after_write = Some(write_id);
                self.pending_connections.insert(generation, pending);
            }
            return Ok(());
        }
        if self.active_client.is_some() {
            if let Ok((write_id, response)) = pending.reject(
                &id,
                REQUEST_CANCELLED,
                "another daemon client is already connected",
            ) {
                self.logger
                    .debug_value("daemon pending client -> ", &response);
                pending.close_after_write = Some(write_id);
                self.pending_connections.insert(generation, pending);
            }
            return Ok(());
        }
        let params = message.get("params").unwrap_or(&Value::Null);
        if let Err(error) = normalize_initialize_params(params, &self.target) {
            if let Ok((write_id, response)) =
                pending.reject(&id, INVALID_REQUEST, &error.to_string())
            {
                self.logger
                    .debug_value("daemon pending client -> ", &response);
                pending.close_after_write = Some(write_id);
                self.pending_connections.insert(generation, pending);
            }
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
