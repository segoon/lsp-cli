use super::process_worker::{ProcessEvent, ProcessSpec, ProcessWorker};
use super::{
    AfterExit, Daemon, DaemonTarget, LifecycleState, UPSTREAM_SHUTDOWN_TIMEOUT, UpstreamServer,
};
use crate::error::{Error, Result};
use crate::lsp::jsonrpc;
use lsp_types::notification::{Exit, Notification};
use lsp_types::request::{Request, Shutdown};
use serde_json::Value;
use std::ffi::OsString;
use std::time::{Duration, Instant};

const SHUTDOWN_ID: &str = "lsp-cli/daemon-shutdown";

impl LifecycleState {
    pub(super) fn is_stopping(self) -> bool {
        matches!(
            self,
            Self::AwaitingShutdownReply {
                after: AfterExit::Stop,
                ..
            } | Self::AwaitingExitWrite {
                after: AfterExit::Stop,
                ..
            } | Self::AwaitingExit {
                after: AfterExit::Stop,
                ..
            } | Self::Killing {
                after: AfterExit::Stop,
                ..
            } | Self::Stopped
        )
    }

    pub(super) fn deadline(self) -> Option<Instant> {
        match self {
            Self::AwaitingShutdownReply { deadline, .. }
            | Self::AwaitingExitWrite { deadline, .. }
            | Self::AwaitingExit { deadline, .. } => Some(deadline),
            _ => None,
        }
    }
}

impl DaemonTarget {
    fn process_spec(&self, debug: bool) -> Result<ProcessSpec> {
        let program = std::env::current_exe().map_err(|error| {
            Error::unexpected(format!("failed to resolve lsp-cli executable: {error}"))
        })?;
        let mut args = vec![
            OsString::from("run"),
            self.path.as_os_str().to_owned(),
            OsString::from("--lsp"),
            OsString::from(&self.server_name),
        ];
        if debug {
            args.push(OsString::from("--debug"));
        }
        Ok(ProcessSpec { program, args })
    }
}

impl Daemon {
    pub(super) fn start_upstream(&mut self, initial: bool) -> Result<()> {
        let generation = self.events.next_generation()?;
        let spec = self.target.process_spec(self.debug)?;
        self.logger.server_starting();
        self.process = Some(ProcessWorker::spawn(spec, generation, &self.events)?);
        self.lifecycle = LifecycleState::Starting {
            generation,
            initial,
        };
        Ok(())
    }

    pub(super) fn wait_for_initial_upstream(&mut self) -> Result<()> {
        while !matches!(self.lifecycle, LifecycleState::Running) {
            let delivery = self.events.receive(None).map_err(|_| {
                Error::unexpected("LSP process worker stopped during daemon startup")
            })?;
            let result = self.dispatch(delivery.event);
            let _ = delivery.acknowledge.send(());
            result?;
        }
        Ok(())
    }

    pub(super) fn handle_process_event(
        &mut self,
        generation: u64,
        event: ProcessEvent,
    ) -> Result<()> {
        if self.process.as_ref().map(ProcessWorker::generation) != Some(generation) {
            return Ok(());
        }
        match event {
            ProcessEvent::Started(io) => {
                if !matches!(
                    self.lifecycle,
                    LifecycleState::Starting {
                        generation: current,
                        ..
                    } if current == generation
                ) {
                    return Ok(());
                }
                self.logger.server_started(io.pid);
                self.upstream = Some(UpstreamServer::from_io(
                    io,
                    generation,
                    self.logger.clone(),
                    &self.events,
                )?);
                self.lifecycle = LifecycleState::Running;
                self.resume_pending_initialize()?;
            }
            ProcessEvent::StartFailed(error) => {
                let initial = matches!(
                    self.lifecycle,
                    LifecycleState::Starting {
                        generation: current,
                        initial: true,
                    } if current == generation
                );
                self.process.take();
                self.lifecycle = LifecycleState::Absent;
                let message = format!(
                    "failed to start LSP server {}: {error}",
                    self.target.server_name
                );
                if initial {
                    return Err(Error::unexpected(message));
                }
                self.logger.unexpected(&message);
                self.fail_pending_initialize(&message)?;
            }
            ProcessEvent::Exited(result) => self.handle_process_exit(generation, result)?,
        }
        Ok(())
    }

    fn handle_process_exit(
        &mut self,
        generation: u64,
        result: std::result::Result<std::process::ExitStatus, String>,
    ) -> Result<()> {
        let after = match self.lifecycle {
            LifecycleState::AwaitingShutdownReply {
                generation: current,
                after,
                ..
            }
            | LifecycleState::AwaitingExitWrite {
                generation: current,
                after,
                ..
            }
            | LifecycleState::AwaitingExit {
                generation: current,
                after,
                ..
            }
            | LifecycleState::Killing {
                generation: current,
                after,
            } if current == generation => after,
            LifecycleState::Running => AfterExit::Absent,
            _ => return Ok(()),
        };
        if let Some(upstream) = self.upstream.take() {
            // Stderr lines are logged by their capture worker. Take a non-waiting snapshot here
            // only to avoid the former coordinator-side flush wait during lifecycle progress.
            let _ = upstream.stderr.summary_now();
        }
        self.process.take();
        match result {
            Ok(status) => self.logger.server_exited(status),
            Err(error) => self.logger.unexpected(error),
        }
        self.orphaned_client_requests.clear();
        match after {
            AfterExit::Restart => self.start_upstream(false)?,
            AfterExit::Stop => self.lifecycle = LifecycleState::Stopped,
            AfterExit::Absent => {
                self.active_client = None;
                self.pending_initialize = None;
                self.lifecycle = LifecycleState::Absent;
                self.idle_since = Instant::now();
            }
        }
        Ok(())
    }

    pub(super) fn begin_restart(&mut self) -> Result<()> {
        match self.lifecycle {
            LifecycleState::Running => self.begin_shutdown(AfterExit::Restart),
            LifecycleState::Absent => self.start_upstream(false),
            _ => Ok(()),
        }
    }

    pub(super) fn begin_stop(&mut self) -> Result<()> {
        self.stop_requested = true;
        self.accept_worker.take();
        self.pending_connections.clear();
        self.disconnect_client()?;
        match self.lifecycle {
            LifecycleState::Running => self.begin_shutdown(AfterExit::Stop),
            LifecycleState::Starting { generation, .. } => {
                self.force_stop(generation, AfterExit::Stop);
                Ok(())
            }
            LifecycleState::Absent => {
                self.lifecycle = LifecycleState::Stopped;
                Ok(())
            }
            LifecycleState::AwaitingShutdownReply {
                generation,
                deadline,
                ..
            } => {
                self.lifecycle = LifecycleState::AwaitingShutdownReply {
                    generation,
                    deadline,
                    after: AfterExit::Stop,
                };
                Ok(())
            }
            LifecycleState::AwaitingExitWrite {
                generation,
                write_id,
                deadline,
                ..
            } => {
                self.lifecycle = LifecycleState::AwaitingExitWrite {
                    generation,
                    write_id,
                    deadline,
                    after: AfterExit::Stop,
                };
                Ok(())
            }
            LifecycleState::AwaitingExit {
                generation,
                deadline,
                ..
            } => {
                self.lifecycle = LifecycleState::AwaitingExit {
                    generation,
                    deadline,
                    after: AfterExit::Stop,
                };
                Ok(())
            }
            LifecycleState::Killing { generation, .. } => {
                self.lifecycle = LifecycleState::Killing {
                    generation,
                    after: AfterExit::Stop,
                };
                Ok(())
            }
            LifecycleState::Stopped => Ok(()),
        }
    }

    fn begin_shutdown(&mut self, after: AfterExit) -> Result<()> {
        let Some(upstream) = self.upstream.as_mut() else {
            self.lifecycle = match after {
                AfterExit::Stop => LifecycleState::Stopped,
                AfterExit::Restart | AfterExit::Absent => LifecycleState::Absent,
            };
            if after == AfterExit::Restart {
                self.start_upstream(false)?;
            }
            return Ok(());
        };
        let generation = upstream.generation;
        if upstream.initialize_fingerprint.is_none() {
            self.force_stop(generation, after);
            return Ok(());
        }
        let shutdown = jsonrpc(
            Some(Value::String(SHUTDOWN_ID.into())),
            Shutdown::METHOD,
            &(),
        )?;
        self.logger.debug_value("daemon upstream <- ", &shutdown);
        if upstream.writer.enqueue(&shutdown).is_err() {
            self.force_stop(generation, after);
            return Ok(());
        }
        self.lifecycle = LifecycleState::AwaitingShutdownReply {
            generation,
            deadline: Instant::now() + UPSTREAM_SHUTDOWN_TIMEOUT,
            after,
        };
        Ok(())
    }

    pub(super) fn handle_lifecycle_message(&mut self, message: &Value) -> Result<bool> {
        let Some(id) = super::response_id(message) else {
            return Ok(false);
        };
        if id != Value::String(SHUTDOWN_ID.into()) {
            return Ok(false);
        }
        let LifecycleState::AwaitingShutdownReply {
            generation, after, ..
        } = self.lifecycle
        else {
            return Ok(true);
        };
        let exit = jsonrpc::<u64, _>(None, Exit::METHOD, &())?;
        self.logger.debug_value("daemon upstream <- ", &exit);
        let Some(upstream) = self.upstream.as_mut() else {
            self.force_stop(generation, after);
            return Ok(true);
        };
        match upstream.writer.enqueue(&exit) {
            Ok(write_id) => {
                self.lifecycle = LifecycleState::AwaitingExitWrite {
                    generation,
                    write_id,
                    deadline: Instant::now() + UPSTREAM_SHUTDOWN_TIMEOUT,
                    after,
                };
            }
            Err(_) => self.force_stop(generation, after),
        }
        Ok(true)
    }

    pub(super) fn handle_lifecycle_write(
        &mut self,
        generation: u64,
        write_id: u64,
        completed_at: Instant,
    ) {
        if let LifecycleState::AwaitingExitWrite {
            generation: current,
            write_id: expected,
            after,
            ..
        } = self.lifecycle
            && current == generation
            && expected == write_id
        {
            self.lifecycle = LifecycleState::AwaitingExit {
                generation,
                deadline: completed_at + UPSTREAM_SHUTDOWN_TIMEOUT,
                after,
            };
        }
    }

    pub(super) fn advance_lifecycle_deadline(&mut self, now: Instant) {
        let Some(deadline) = self.lifecycle.deadline() else {
            return;
        };
        if now < deadline {
            return;
        }
        let (LifecycleState::AwaitingShutdownReply {
            generation, after, ..
        }
        | LifecycleState::AwaitingExitWrite {
            generation, after, ..
        }
        | LifecycleState::AwaitingExit {
            generation, after, ..
        }) = self.lifecycle
        else {
            return;
        };
        self.force_stop(generation, after);
    }

    pub(super) fn lifecycle_timeout(&self, now: Instant) -> Option<Duration> {
        self.lifecycle
            .deadline()
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    pub(super) fn upstream_failed(&mut self) {
        let Some(generation) = self.upstream.as_ref().map(|upstream| upstream.generation) else {
            return;
        };
        let after = match self.lifecycle {
            LifecycleState::AwaitingShutdownReply { after, .. }
            | LifecycleState::AwaitingExitWrite { after, .. }
            | LifecycleState::AwaitingExit { after, .. }
            | LifecycleState::Killing { after, .. } => after,
            _ => AfterExit::Absent,
        };
        if after == AfterExit::Absent {
            self.active_client = None;
            self.pending_initialize = None;
        }
        self.force_stop(generation, after);
    }

    fn force_stop(&mut self, generation: u64, after: AfterExit) {
        if let Some(process) = self.process.as_ref()
            && let Err(error) = process.force_stop()
        {
            self.logger.unexpected(error);
        }
        self.lifecycle = LifecycleState::Killing { generation, after };
    }
}

impl UpstreamServer {
    pub(super) fn from_io(
        io: super::process_worker::ProcessIo,
        generation: u64,
        logger: super::Logger,
        events: &super::events::EventQueue,
    ) -> Result<Self> {
        let stderr = crate::server_stderr::CapturedStderr::spawn_with(io.stderr, logger);
        let reader = super::events::ReaderWorker::spawn(
            io.stdout,
            super::events::Source::Upstream(generation),
            events,
        )?;
        let writer = super::writer::WriterWorker::spawn(
            io.stdin,
            super::events::Source::Upstream(generation),
            events,
        )?;
        Ok(Self {
            writer,
            stderr,
            generation,
            _reader: reader,
            initialize_fingerprint: None,
            initialize_result: None,
            restart_required: false,
            background_work: super::BackgroundWorkTracker::default(),
        })
    }
}
