use super::events::{Event, EventQueue, Source};
use crate::error::{Error, Result};
use crate::lsp::transport::frame_message;
use serde_json::Value;
use std::io::Write;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(super) const MESSAGE_LIMIT: usize = 64;
pub(super) const BYTE_LIMIT: usize = 8 * 1024 * 1024;

pub(super) type WriteId = u64;

#[derive(Debug)]
pub(super) enum WriterEvent {
    Completed { id: WriteId, completed_at: Instant },
    Failed { id: WriteId, error: String },
}

struct Frame {
    id: WriteId,
    bytes: Vec<u8>,
}

struct Progress {
    messages: AtomicUsize,
    bytes: AtomicUsize,
}

pub(super) struct WriterWorker {
    sender: Option<Sender<Frame>>,
    progress: Arc<Progress>,
    flagged_since: Option<Instant>,
    next_id: WriteId,
    socket: Option<UnixStream>,
    cancelled: Arc<AtomicBool>,
    acknowledge: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl WriterWorker {
    pub(super) fn spawn<W: Write + Send + 'static>(
        mut writer: W,
        source: Source,
        events: &EventQueue,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel::<Frame>();
        let progress = Arc::new(Progress {
            messages: AtomicUsize::new(0),
            bytes: AtomicUsize::new(0),
        });
        let worker_progress = Arc::clone(&progress);
        let (cancelled, acknowledge, admission) = events.admission();
        let handle = thread::Builder::new()
            .name("daemon-writer".to_string())
            .spawn(move || {
                while let Ok(frame) = receiver.recv() {
                    let result = writer.write_all(&frame.bytes).and_then(|()| writer.flush());
                    let failed = result.is_err();
                    worker_progress.messages.fetch_sub(1, Ordering::AcqRel);
                    worker_progress
                        .bytes
                        .fetch_sub(frame.bytes.len(), Ordering::AcqRel);
                    let event = match result {
                        Ok(()) => WriterEvent::Completed {
                            id: frame.id,
                            completed_at: Instant::now(),
                        },
                        Err(error) => WriterEvent::Failed {
                            id: frame.id,
                            error: error.to_string(),
                        },
                    };
                    // Completion must not hold up the next write; progress atomics bound the
                    // coordinator's view while events report ordering and failures.
                    if !admission.publish(Event::Writer(source, event)) || failed {
                        return;
                    }
                }
            })
            .map_err(|error| {
                Error::unexpected(format!("failed to start daemon writer: {error}"))
            })?;
        Ok(Self {
            sender: Some(sender),
            progress,
            flagged_since: None,
            next_id: 0,
            socket: None,
            cancelled,
            acknowledge,
            thread: Some(handle),
        })
    }

    pub(super) fn socket(socket: UnixStream, source: Source, events: &EventQueue) -> Result<Self> {
        let writer = socket.try_clone().map_err(|error| {
            Error::unexpected(format!("failed to clone daemon client writer: {error}"))
        })?;
        let mut worker = Self::spawn(writer, source, events)?;
        worker.socket = Some(socket);
        Ok(worker)
    }

    pub(super) fn enqueue(&mut self, message: &Value) -> Result<WriteId> {
        let frame = frame_message(message)?;
        let messages = self
            .progress
            .messages
            .load(Ordering::Acquire)
            .checked_add(1)
            .ok_or_else(|| Error::unexpected("daemon output message count overflowed"))?;
        let bytes = self
            .progress
            .bytes
            .load(Ordering::Acquire)
            .checked_add(frame.len())
            .ok_or_else(|| Error::unexpected("daemon output byte count overflowed"))?;
        let Some(id) = self.next_id.checked_add(1) else {
            return Err(Error::unexpected(
                "daemon exhausted its output message identifiers",
            ));
        };
        self.next_id = id;
        self.progress.messages.fetch_add(1, Ordering::AcqRel);
        self.progress.bytes.fetch_add(frame.len(), Ordering::AcqRel);
        let Some(sender) = self.sender.as_ref() else {
            self.rollback(frame.len());
            return Err(Error::lsp("daemon output is closed"));
        };
        if let Err(error) = sender.send(Frame { id, bytes: frame }) {
            self.rollback(error.0.bytes.len());
            return Err(Error::lsp("daemon output writer stopped"));
        }
        if (messages >= MESSAGE_LIMIT || bytes >= BYTE_LIMIT) && self.flagged_since.is_none() {
            self.flagged_since = Some(Instant::now());
        }
        Ok(id)
    }

    fn rollback(&self, bytes: usize) {
        self.progress.messages.fetch_sub(1, Ordering::AcqRel);
        self.progress.bytes.fetch_sub(bytes, Ordering::AcqRel);
    }

    pub(super) fn refresh_flag(&mut self, now: Instant) {
        let messages = self.progress.messages.load(Ordering::Acquire);
        let bytes = self.progress.bytes.load(Ordering::Acquire);
        if messages < MESSAGE_LIMIT && bytes < BYTE_LIMIT {
            self.flagged_since = None;
        } else if self.flagged_since.is_none() {
            self.flagged_since = Some(now);
        }
    }

    pub(super) fn deadline(&self, timeout: Duration) -> Option<Instant> {
        self.flagged_since
            .and_then(|started| started.checked_add(timeout))
    }

    pub(super) fn timed_out(&mut self, now: Instant, timeout: Duration) -> bool {
        self.refresh_flag(now);
        self.deadline(timeout)
            .is_some_and(|deadline| now >= deadline)
    }

    #[cfg(test)]
    pub(super) fn outstanding(&self) -> (usize, usize) {
        (
            self.progress.messages.load(Ordering::Acquire),
            self.progress.bytes.load(Ordering::Acquire),
        )
    }

    pub(super) fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.acknowledge.send(());
        self.sender.take();
        if let Some(socket) = &self.socket {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}

impl Drop for WriterWorker {
    fn drop(&mut self) {
        // A socket shutdown interrupts a blocked write. Child pipes have no equivalent safe
        // standard-library operation, so never impose an unlimited join on their writer.
        self.cancel();
        if let Some(thread) = self.thread.take()
            && (self.socket.is_some() || thread.is_finished())
        {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests;
