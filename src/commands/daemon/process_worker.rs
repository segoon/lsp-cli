use super::events::{Event, EventQueue};
use crate::error::{Error, Result};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(super) struct ProcessSpec {
    pub(super) program: PathBuf,
    pub(super) args: Vec<OsString>,
}

pub(super) struct ProcessIo {
    pub(super) stdin: ChildStdin,
    pub(super) stdout: ChildStdout,
    pub(super) stderr: ChildStderr,
    pub(super) pid: u32,
}

pub(super) enum ProcessEvent {
    Started(ProcessIo),
    StartFailed(String),
    Exited(std::result::Result<ExitStatus, String>),
}

enum ProcessCommand {
    ForceStop,
}

pub(super) struct ProcessWorker {
    generation: u64,
    commands: Option<Sender<ProcessCommand>>,
    cancelled: Arc<AtomicBool>,
}

impl ProcessWorker {
    pub(super) fn spawn(spec: ProcessSpec, generation: u64, events: &EventQueue) -> Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let (cancelled, _, admission) = events.admission();
        let worker_cancelled = Arc::clone(&cancelled);
        thread::Builder::new()
            .name("daemon-process".into())
            .spawn(move || {
                let mut command = Command::new(&spec.program);
                command
                    .args(&spec.args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(error) => {
                        let _ = admission.publish(Event::Process(
                            generation,
                            ProcessEvent::StartFailed(error.to_string()),
                        ));
                        return;
                    }
                };

                if matches!(receiver.try_recv(), Ok(ProcessCommand::ForceStop)) {
                    publish_exit(generation, stop_child(&mut child), &admission);
                    return;
                }
                let Some(io) = take_io(&mut child) else {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = admission.publish(Event::Process(
                        generation,
                        ProcessEvent::StartFailed(
                            "LSP server process did not provide standard I/O".into(),
                        ),
                    ));
                    return;
                };
                if !admission.publish(Event::Process(generation, ProcessEvent::Started(io))) {
                    let _ = stop_child(&mut child);
                    return;
                }

                loop {
                    match receiver.try_recv() {
                        Ok(ProcessCommand::ForceStop) => {
                            publish_exit(generation, stop_child(&mut child), &admission);
                            return;
                        }
                        Err(TryRecvError::Disconnected) => {
                            let _ = stop_child(&mut child);
                            return;
                        }
                        Err(TryRecvError::Empty) => {}
                    }
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            publish_exit(generation, Ok(status), &admission);
                            return;
                        }
                        Ok(None) => thread::sleep(PROCESS_POLL_INTERVAL),
                        Err(error) => {
                            publish_exit(generation, Err(error.to_string()), &admission);
                            return;
                        }
                    }
                    if worker_cancelled.load(Ordering::Acquire) {
                        let _ = stop_child(&mut child);
                        return;
                    }
                }
            })
            .map_err(|error| {
                Error::unexpected(format!("failed to start LSP process worker: {error}"))
            })?;
        Ok(Self {
            generation,
            commands: Some(commands),
            cancelled,
        })
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg(test)]
    pub(super) fn adopt(
        mut child: Child,
        generation: u64,
        events: &EventQueue,
    ) -> (Self, ProcessIo) {
        let io = take_io(&mut child).expect("test process standard I/O");
        let (commands, receiver) = mpsc::channel();
        let (cancelled, _, admission) = events.admission();
        let worker_cancelled = Arc::clone(&cancelled);
        thread::Builder::new()
            .name("daemon-test-process".into())
            .spawn(move || {
                loop {
                    match receiver.try_recv() {
                        Ok(ProcessCommand::ForceStop) => {
                            publish_exit(generation, stop_child(&mut child), &admission);
                            return;
                        }
                        Err(TryRecvError::Disconnected) => {
                            let _ = stop_child(&mut child);
                            return;
                        }
                        Err(TryRecvError::Empty) => {}
                    }
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            publish_exit(generation, Ok(status), &admission);
                            return;
                        }
                        Ok(None) => thread::sleep(PROCESS_POLL_INTERVAL),
                        Err(error) => {
                            publish_exit(generation, Err(error.to_string()), &admission);
                            return;
                        }
                    }
                    if worker_cancelled.load(Ordering::Acquire) {
                        let _ = stop_child(&mut child);
                        return;
                    }
                }
            })
            .expect("test process worker");
        (
            Self {
                generation,
                commands: Some(commands),
                cancelled,
            },
            io,
        )
    }

    pub(super) fn force_stop(&self) -> Result<()> {
        self.commands
            .as_ref()
            .ok_or_else(|| Error::unexpected("LSP process worker is closed"))?
            .send(ProcessCommand::ForceStop)
            .map_err(|_| Error::unexpected("LSP process worker stopped before termination"))
    }
}

impl Drop for ProcessWorker {
    fn drop(&mut self) {
        // The worker owns Child, so request termination explicitly before closing its command
        // channel. Joining here would put an unlimited process wait back on the coordinator.
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(ProcessCommand::ForceStop);
        }
        self.cancelled.store(true, Ordering::Release);
    }
}

fn take_io(child: &mut Child) -> Option<ProcessIo> {
    Some(ProcessIo {
        stdin: child.stdin.take()?,
        stdout: child.stdout.take()?,
        stderr: child.stderr.take()?,
        pid: child.id(),
    })
}

fn stop_child(child: &mut Child) -> std::result::Result<ExitStatus, String> {
    match child.try_wait() {
        Ok(Some(status)) => return Ok(status),
        Ok(None) => {}
        Err(error) => return Err(format!("failed to inspect LSP server process: {error}")),
    }
    let kill_error = child.kill().err();
    match child.wait() {
        Ok(status) => Ok(status),
        Err(wait_error) => Err(match kill_error {
            Some(kill_error) => format!(
                "failed to stop LSP server process: {kill_error}; failed to reap it: {wait_error}"
            ),
            None => format!("failed to reap LSP server process: {wait_error}"),
        }),
    }
}

fn publish_exit(
    generation: u64,
    result: std::result::Result<ExitStatus, String>,
    admission: &super::events::Admission,
) {
    let _ = admission.publish(Event::Process(generation, ProcessEvent::Exited(result)));
}
