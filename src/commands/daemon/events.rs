use super::protocol::ReaderEvent;
use crate::error::{Error, Result};
use crate::lsp::transport::read_message;
use crate::system_log::log_unexpected_error;
use std::collections::VecDeque;
use std::io::{BufReader, Read};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Source {
    Client(u64),
    Upstream(u64),
}

pub(super) enum Event {
    Accepted(UnixStream),
    AcceptError(String),
    Reader(Source, ReaderEvent),
}

pub(super) struct Delivery {
    pub(super) event: Event,
    pub(super) acknowledge: Sender<()>,
}

pub(super) struct EventQueue {
    sender: Sender<Delivery>,
    receiver: Receiver<Delivery>,
    deferred: VecDeque<Delivery>,
    generation: u64,
}

impl EventQueue {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            deferred: VecDeque::new(),
            generation: 0,
        }
    }

    pub(super) fn next_generation(&mut self) -> Result<u64> {
        let Some(generation) = self.generation.checked_add(1) else {
            return Err(Error::unexpected(
                "daemon exhausted its connection identifiers; restart the daemon",
            ));
        };
        self.generation = generation;
        Ok(self.generation)
    }

    pub(super) fn receive(
        &mut self,
        timeout: Option<Duration>,
    ) -> std::result::Result<Delivery, RecvTimeoutError> {
        if let Some(event) = self.deferred.pop_front() {
            return Ok(event);
        }
        match timeout {
            Some(timeout) => self.receiver.recv_timeout(timeout),
            None => self
                .receiver
                .recv()
                .map_err(|_| RecvTimeoutError::Disconnected),
        }
    }

    pub(super) fn receive_upstream(
        &mut self,
        generation: u64,
        timeout: Duration,
    ) -> std::result::Result<ReaderEvent, RecvTimeoutError> {
        let started = Instant::now();
        loop {
            let remaining = timeout
                .checked_sub(started.elapsed())
                .ok_or(RecvTimeoutError::Timeout)?;
            let delivery = self.receiver.recv_timeout(remaining)?;
            match delivery.event {
                Event::Reader(Source::Upstream(id), event) if id == generation => {
                    let _ = delivery.acknowledge.send(());
                    return Ok(event);
                }
                event => {
                    // Withhold admission while shutdown is synchronous. At most one event per
                    // other producer can be deferred, and normal dispatch retains their order.
                    self.deferred.push_back(Delivery {
                        event,
                        acknowledge: delivery.acknowledge,
                    });
                }
            }
        }
    }
}

struct Admission {
    cancelled: Arc<AtomicBool>,
    acknowledge: Sender<()>,
    replies: Receiver<()>,
    events: Sender<Delivery>,
}

impl Admission {
    fn deliver(&self, event: Event) -> bool {
        if self.cancelled.load(Ordering::Acquire) {
            return false;
        }
        // Only this producer uses replies: it cannot read/accept again until coordination
        // completes this event. The shared FIFO therefore needs no blocking coordinator send.
        if self
            .events
            .send(Delivery {
                event,
                acknowledge: self.acknowledge.clone(),
            })
            .is_err()
        {
            return false;
        }
        self.replies.recv().is_ok() && !self.cancelled.load(Ordering::Acquire)
    }
}

pub(super) struct ReaderWorker {
    cancelled: Arc<AtomicBool>,
    acknowledge: Sender<()>,
    socket: Option<UnixStream>,
    thread: Option<JoinHandle<()>>,
}

impl ReaderWorker {
    fn admission(events: &EventQueue) -> (Self, Admission) {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (acknowledge, replies) = mpsc::channel();
        let admission = Admission {
            cancelled: Arc::clone(&cancelled),
            acknowledge: acknowledge.clone(),
            replies,
            events: events.sender.clone(),
        };
        (
            Self {
                cancelled,
                acknowledge,
                socket: None,
                thread: None,
            },
            admission,
        )
    }

    pub(super) fn spawn<R: Read + Send + 'static>(
        reader: R,
        source: Source,
        events: &EventQueue,
    ) -> Result<Self> {
        let (mut worker, admission) = Self::admission(events);
        worker.thread = Some(
            thread::Builder::new()
                .name("daemon-reader".into())
                .spawn(move || {
                    let mut reader = BufReader::new(reader);
                    while !admission.cancelled.load(Ordering::Acquire) {
                        let event = match read_message(&mut reader) {
                            Ok(Some(message)) => ReaderEvent::Message(message),
                            Ok(None) => ReaderEvent::EndOfStream,
                            Err(error) => ReaderEvent::Error(error.to_string()),
                        };
                        let terminal = !matches!(event, ReaderEvent::Message(_));
                        if !admission.deliver(Event::Reader(source, event)) || terminal {
                            return;
                        }
                    }
                })
                .map_err(|error| {
                    Error::unexpected(format!("failed to start daemon reader: {error}"))
                })?,
        );
        Ok(worker)
    }

    pub(super) fn socket(socket: UnixStream, source: Source, events: &EventQueue) -> Result<Self> {
        let reader = socket.try_clone().map_err(|error| {
            Error::unexpected(format!("failed to clone daemon client socket: {error}"))
        })?;
        let mut worker = Self::spawn(reader, source, events)?;
        worker.socket = Some(socket);
        Ok(worker)
    }

    pub(super) fn cancel(&self) -> bool {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.acknowledge.send(());
        if let Some(socket) = &self.socket {
            return socket.shutdown(Shutdown::Both).is_ok();
        }
        false
    }
}

impl Drop for ReaderWorker {
    fn drop(&mut self) {
        // Admission can be blocked even after the peer closes; socket shutdown also interrupts
        // partial-frame reads. Inherited child pipes cannot safely be joined until EOF arrives.
        let interrupted = self.cancel();
        if let Some(thread) = self.thread.take()
            && (interrupted || thread.is_finished())
        {
            let _ = thread.join();
        }
    }
}

pub(super) struct AcceptWorker {
    worker: ReaderWorker,
    socket_path: PathBuf,
}

impl AcceptWorker {
    pub(super) fn spawn(
        listener: UnixListener,
        socket_path: &Path,
        events: &EventQueue,
    ) -> Result<Self> {
        let (mut worker, admission) = ReaderWorker::admission(events);
        worker.thread = Some(
            thread::Builder::new()
                .name("daemon-accept".into())
                .spawn(move || {
                    while !admission.cancelled.load(Ordering::Acquire) {
                        let event = match listener.accept() {
                            Ok((stream, _)) => Event::Accepted(stream),
                            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                                continue;
                            }
                            Err(error) => Event::AcceptError(error.to_string()),
                        };
                        let terminal = matches!(event, Event::AcceptError(_));
                        if !admission.deliver(event) || terminal {
                            return;
                        }
                    }
                })
                .map_err(|error| {
                    Error::unexpected(format!("failed to start daemon listener: {error}"))
                })?,
        );
        Ok(Self {
            worker,
            socket_path: socket_path.to_owned(),
        })
    }
}

impl Drop for AcceptWorker {
    fn drop(&mut self) {
        // Cancellation releases admission; a self-connection wakes blocking accept. Check the
        // cancellation flag before publishing so this wakeup never becomes a client session.
        self.worker.cancel();
        let wakeup = UnixStream::connect(&self.socket_path);
        if let Some(thread) = self.worker.thread.take() {
            if wakeup.is_ok() || thread.is_finished() {
                let _ = thread.join();
            } else {
                // An externally removed/replaced socket may no longer reach our listener.
                // Do not turn that error into an unbounded join during daemon teardown.
                log_unexpected_error("could not wake daemon listener during shutdown");
            }
        }
    }
}

#[cfg(test)]
mod tests;
