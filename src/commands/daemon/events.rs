use super::protocol::ReaderEvent;
use super::socket_reader::SocketReader;
use crate::error::{Error, Result};
use crate::lsp::transport::read_message;
use crate::system_log::log_unexpected_error;
use serde_json::Value;
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
    Accepted {
        stream: UnixStream,
        accepted_at: Instant,
    },
    AcceptError(String),
    Reader(Source, ReaderEvent),
    Writer(Source, super::writer::WriterEvent),
    Process(u64, super::process_worker::ProcessEvent),
}

pub(super) struct Delivery {
    pub(super) event: Event,
    pub(super) acknowledge: Sender<()>,
}

pub(super) struct EventQueue {
    sender: Sender<Delivery>,
    receiver: Receiver<Delivery>,
    generation: u64,
}

impl EventQueue {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
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

    pub(super) fn admission(&self) -> (Arc<AtomicBool>, Sender<()>, Admission) {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (acknowledge, replies) = mpsc::channel();
        let admission = Admission {
            cancelled: Arc::clone(&cancelled),
            acknowledge: acknowledge.clone(),
            replies,
            events: self.sender.clone(),
        };
        (cancelled, acknowledge, admission)
    }

    pub(super) fn receive(
        &mut self,
        timeout: Option<Duration>,
    ) -> std::result::Result<Delivery, RecvTimeoutError> {
        match timeout {
            Some(timeout) => self.receiver.recv_timeout(timeout),
            None => self
                .receiver
                .recv()
                .map_err(|_| RecvTimeoutError::Disconnected),
        }
    }
}

pub(super) struct Admission {
    cancelled: Arc<AtomicBool>,
    acknowledge: Sender<()>,
    replies: Receiver<()>,
    events: Sender<Delivery>,
}

impl Admission {
    pub(super) fn publish(&self, event: Event) -> bool {
        if self.cancelled.load(Ordering::Acquire) {
            return false;
        }
        let (acknowledge, replies) = mpsc::channel();
        drop(replies);
        self.events.send(Delivery { event, acknowledge }).is_ok()
    }

    pub(super) fn deliver(&self, event: Event) -> bool {
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
    pub(super) fn admission(events: &EventQueue) -> (Self, Admission) {
        let (cancelled, acknowledge, admission) = events.admission();
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
        let mut reader = BufReader::new(reader);
        Self::spawn_messages(move || read_message(&mut reader), source, events)
    }

    fn spawn_messages(
        mut next_message: impl FnMut() -> Result<Option<Value>> + Send + 'static,
        source: Source,
        events: &EventQueue,
    ) -> Result<Self> {
        let (mut worker, admission) = Self::admission(events);
        worker.thread = Some(
            thread::Builder::new()
                .name("daemon-reader".into())
                .spawn(move || {
                    while !admission.cancelled.load(Ordering::Acquire) {
                        let event = match next_message() {
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

    #[cfg(test)]
    pub(super) fn socket(socket: UnixStream, source: Source, events: &EventQueue) -> Result<Self> {
        Self::socket_with_deadline(socket, source, events, None)
    }

    pub(super) fn socket_with_deadline(
        socket: UnixStream,
        source: Source,
        events: &EventQueue,
        deadline: Option<Instant>,
    ) -> Result<Self> {
        let reader = socket.try_clone().map_err(|error| {
            Error::unexpected(format!("failed to clone daemon client socket: {error}"))
        })?;
        let mut reader = SocketReader::new(reader, deadline);
        let mut worker = Self::spawn_messages(move || reader.next_message(), source, events)?;
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
                            Ok((stream, _)) => Event::Accepted {
                                stream,
                                accepted_at: Instant::now(),
                            },
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
