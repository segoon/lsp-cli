use crate::error::{Error, Result};
use crate::lsp::transport::serialize_debug_message;
use crate::server_stderr::StderrSink;
use crate::system_log::{append_system_log_line, format_exit_status};
use serde_json::Value;
use std::io::Write;
use std::process::ExitStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const LOG_QUEUE_LIMIT: usize = 64;
const WORKER_WAKE_INTERVAL: Duration = Duration::from_millis(5);
pub(super) const LOG_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(100);

enum LogRecord {
    Debug {
        prefix: &'static str,
        message: Arc<Value>,
    },
    System(String),
    Stderr(Vec<u8>),
}

trait LogDestination: Send + 'static {
    fn write(&mut self, record: LogRecord);
    fn report_dropped(&mut self, count: usize);
}

struct ProductionDestination {
    debug: bool,
}

impl LogDestination for ProductionDestination {
    fn write(&mut self, record: LogRecord) {
        match record {
            LogRecord::Debug { prefix, message } if self.debug => {
                eprintln!("{prefix}{}", serialize_debug_message(&message));
            }
            LogRecord::System(message) => append_system_log_line(&message),
            LogRecord::Stderr(chunk) if self.debug => {
                let mut stderr = std::io::stderr().lock();
                let _ = stderr.write_all(&chunk);
                let _ = stderr.flush();
            }
            LogRecord::Debug { .. } | LogRecord::Stderr(_) => {}
        }
    }

    fn report_dropped(&mut self, count: usize) {
        let message = format!(
            "daemon logger dropped {count} diagnostic record(s) because its queue was full"
        );
        append_system_log_line(&message);
        if self.debug {
            eprintln!("{message}");
        }
    }
}

struct Shared {
    accepting: AtomicBool,
    shutdown: AtomicBool,
    dropped: AtomicUsize,
}

#[derive(Clone)]
pub(super) struct Logger {
    sender: SyncSender<LogRecord>,
    shared: Arc<Shared>,
    debug: bool,
}

impl Logger {
    fn enqueue(&self, record: LogRecord) {
        if !self.shared.accepting.load(Ordering::Acquire) {
            return;
        }
        match self.sender.try_send(record) {
            Err(TrySendError::Full(_)) => increment_saturating(&self.shared.dropped),
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
        }
    }

    pub(super) fn debug(&self, prefix: &'static str, message: Arc<Value>) {
        if self.debug {
            self.enqueue(LogRecord::Debug { prefix, message });
        }
    }

    pub(super) fn debug_value(&self, prefix: &'static str, message: &Value) {
        if self.debug {
            self.enqueue(LogRecord::Debug {
                prefix,
                message: Arc::new(message.clone()),
            });
        }
    }

    pub(super) fn system(&self, message: impl Into<String>) {
        self.enqueue(LogRecord::System(message.into()));
    }

    pub(super) fn unexpected(&self, error: impl std::fmt::Display) {
        self.system(format!("unexpected error: {error}"));
    }

    pub(super) fn server_starting(&self) {
        self.system("starting LSP server...");
    }

    pub(super) fn server_started(&self, pid: u32) {
        self.system(format!("LSP server has started (pid {pid})"));
    }

    pub(super) fn server_exited(&self, status: ExitStatus) {
        self.system(format!(
            "LSP server exited with {}",
            format_exit_status(status)
        ));
    }

    #[cfg(test)]
    fn dropped(&self) -> usize {
        self.shared.dropped.load(Ordering::Acquire)
    }
}

impl StderrSink for Logger {
    fn write_chunk(&self, chunk: &[u8]) {
        if self.debug {
            self.enqueue(LogRecord::Stderr(chunk.to_vec()));
        }
    }

    fn write_line(&self, line: String) {
        self.system(format!("stderr: {line}"));
    }

    fn write_error(&self, error: String) {
        self.unexpected(error);
    }
}

pub(super) struct LoggerWorker {
    logger: Logger,
    finished: Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

impl LoggerWorker {
    pub(super) fn spawn(debug: bool) -> Result<Self> {
        Self::spawn_with(
            debug,
            Box::new(ProductionDestination { debug }),
            LOG_QUEUE_LIMIT,
        )
    }

    fn spawn_with(
        debug: bool,
        mut destination: Box<dyn LogDestination>,
        queue_limit: usize,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(queue_limit);
        let shared = Arc::new(Shared {
            accepting: AtomicBool::new(true),
            shutdown: AtomicBool::new(false),
            dropped: AtomicUsize::new(0),
        });
        let worker_shared = Arc::clone(&shared);
        let (finished_sender, finished) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("daemon-logger".into())
            .spawn(move || {
                run_worker(&receiver, &worker_shared, &mut *destination);
                let _ = finished_sender.send(());
            })
            .map_err(|error| {
                Error::unexpected(format!("failed to start daemon logger: {error}"))
            })?;
        Ok(Self {
            logger: Logger {
                sender,
                shared,
                debug,
            },
            finished,
            thread: Some(thread),
        })
    }

    pub(super) fn logger(&self) -> Logger {
        self.logger.clone()
    }

    pub(super) fn finish(&mut self, timeout: Duration) {
        if self.thread.is_none() {
            return;
        }
        self.signal_shutdown();
        if self.finished.recv_timeout(timeout).is_ok()
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }

    fn signal_shutdown(&self) {
        self.logger.shared.accepting.store(false, Ordering::Release);
        self.logger.shared.shutdown.store(true, Ordering::Release);
    }
}

impl Drop for LoggerWorker {
    fn drop(&mut self) {
        // Error paths must stop accepting logs, but waiting here could put an unlimited file-lock
        // delay back on daemon cleanup. Normal shutdown calls finish with an explicit deadline.
        self.signal_shutdown();
        if self
            .thread
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

fn run_worker(receiver: &Receiver<LogRecord>, shared: &Shared, output: &mut dyn LogDestination) {
    loop {
        if shared.shutdown.load(Ordering::Acquire) {
            while let Ok(record) = receiver.try_recv() {
                output.write(record);
                report_dropped(shared, output);
            }
            report_dropped(shared, output);
            return;
        }
        match receiver.recv_timeout(WORKER_WAKE_INTERVAL) {
            Ok(record) => {
                output.write(record);
                report_dropped(shared, output);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn report_dropped(shared: &Shared, output: &mut dyn LogDestination) {
    let dropped = shared.dropped.swap(0, Ordering::AcqRel);
    if dropped > 0 {
        output.report_dropped(dropped);
    }
}

fn increment_saturating(value: &AtomicUsize) {
    let _ = value.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
        Some(current.saturating_add(1))
    });
}

#[cfg(test)]
mod tests;
