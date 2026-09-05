use super::Daemon;
use super::events::Source;
use super::writer::WriterEvent;
use crate::error::Result;
use crate::system_log::log_unexpected_error;
use std::time::Instant;

impl Daemon {
    pub(super) fn handle_writer_event(&mut self, source: Source, event: WriterEvent) -> Result<()> {
        match (source, event) {
            (Source::Client(generation), WriterEvent::Completed { id, completed_at }) => {
                if let Some(pending) = self.pending_connections.get_mut(&generation) {
                    pending.client.writer.refresh_flag(completed_at);
                    if pending.close_after_write == Some(id) {
                        let stop = pending.stop_after_write;
                        self.pending_connections.remove(&generation);
                        self.stop_requested |= stop;
                    }
                } else if let Some(client) = self.active_client.as_mut()
                    && client.generation == generation
                {
                    client.writer.refresh_flag(completed_at);
                    if client.stop_after_write == Some(id) {
                        self.stop_requested = true;
                    }
                    if client.disconnect_after_write == Some(id) {
                        self.disconnect_client()?;
                    }
                }
            }
            (Source::Client(generation), WriterEvent::Failed { id, error }) => {
                log_unexpected_error(&format!(
                    "daemon client output failed while writing message {id}: {error}"
                ));
                if self.pending_connections.remove(&generation).is_none()
                    && self
                        .active_client
                        .as_ref()
                        .is_some_and(|client| client.generation == generation)
                {
                    self.disconnect_client()?;
                }
            }
            (Source::Upstream(generation), WriterEvent::Completed { id, completed_at }) => {
                self.handle_lifecycle_write(generation, id, completed_at);
                if let Some(upstream) = self.upstream.as_mut()
                    && upstream.generation == generation
                {
                    upstream.writer.refresh_flag(completed_at);
                }
            }
            (Source::Upstream(generation), WriterEvent::Failed { id, error }) => {
                if self
                    .upstream
                    .as_ref()
                    .is_some_and(|upstream| upstream.generation == generation)
                {
                    log_unexpected_error(&format!(
                        "LSP server stopped accepting message {id}: {error}"
                    ));
                    self.upstream_failed();
                }
            }
        }
        Ok(())
    }

    pub(super) fn expire_stalled_outputs(&mut self, now: Instant) -> Result<()> {
        let stalled_pending: Vec<_> = self
            .pending_connections
            .iter_mut()
            .filter_map(|(generation, pending)| {
                pending
                    .client
                    .writer
                    .timed_out(now, self.write_stall_timeout)
                    .then_some(*generation)
            })
            .collect();
        for generation in stalled_pending {
            self.pending_connections.remove(&generation);
        }
        if self
            .active_client
            .as_mut()
            .is_some_and(|client| client.writer.timed_out(now, self.write_stall_timeout))
        {
            log_unexpected_error(
                "daemon client was disconnected because it stopped reading output",
            );
            self.disconnect_client()?;
        }
        if self
            .upstream
            .as_mut()
            .is_some_and(|upstream| upstream.writer.timed_out(now, self.write_stall_timeout))
        {
            log_unexpected_error("LSP server was stopped because it stopped reading daemon output");
            self.upstream_failed();
        }
        Ok(())
    }
}
