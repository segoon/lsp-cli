use crate::cli::DaemonArgs;
use crate::config::ConfigStore;
use crate::error::{Error, Result, error_fn};
use crate::lsp::transport::{log_debug_message, write_message};
use crate::lsp::{jsonrpc, parse_lsp_uri};
use crate::server_stderr::CapturedStderr;
use crate::system_log::{log_lsp_server_exit, log_unexpected_error};
use lsp_types::notification::{Cancel, DidCloseTextDocument, Notification};
use lsp_types::{CancelParams, DidCloseTextDocumentParams, NumberOrString, TextDocumentIdentifier};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, ChildStdin};
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::{Duration, Instant};

mod events;
mod forwarding;
mod process;
mod protocol;

#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod tests;

use events::{AcceptWorker, Event, EventQueue, ReaderWorker, Source};
use process::{bind_listener, launch_background, resolve_target, run_background};
use protocol::{
    BackgroundWorkTracker, ReaderEvent, error_response, handle_busy_connection,
    read_control_message, request_id_from_key, response_id, stop_request, stop_request_id,
};

const BACKGROUND_ENV: &str = "LSP_CLI_DAEMON_BACKGROUND";
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const BUSY_CLIENT_TIMEOUT: Duration = Duration::from_millis(250);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(2);
const DETACHED_IDLE_TIMEOUT: Duration = Duration::from_mins(1);
const STOP_COMPLETION_TIMEOUT: Duration = Duration::from_secs(5);
const UPSTREAM_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const SERVER_NOT_INITIALIZED: i64 = -32002;
const INVALID_REQUEST: i64 = -32600;
const REQUEST_CANCELLED: i64 = -32800;

pub(super) fn run(args: &DaemonArgs, config: &ConfigStore) -> Result<String> {
    let target = resolve_target(args, config)?;

    if std::env::var_os(BACKGROUND_ENV).is_some() {
        return run_background(args, target);
    }

    launch_background(args, &target)
}

pub(super) fn launch_for_workspace(
    workspace_root: &Path,
    server_name: &str,
    socket_path: &Path,
    debug: bool,
) -> Result<()> {
    process::launch_background_for_connection(
        workspace_root,
        server_name,
        socket_path,
        debug,
        DETACHED_IDLE_TIMEOUT,
    )
}

pub(super) enum StopSocketResult {
    Stopped,
    RemovedStaleSocket,
    NotRunning,
}

pub(super) fn stop_socket(socket_path: &Path, debug: bool) -> Result<StopSocketResult> {
    if !socket_path.exists() {
        return Ok(StopSocketResult::NotRunning);
    }

    let mut stream = match UnixStream::connect(socket_path) {
        Ok(stream) => stream,
        Err(connect_error) => match fs::remove_file(socket_path) {
            Ok(()) => return Ok(StopSocketResult::RemovedStaleSocket),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(StopSocketResult::NotRunning);
            }
            Err(error) => {
                return Err(Error::unexpected(format!(
                    "failed to connect to daemon socket {}: {connect_error}; failed to remove stale socket: {error}",
                    socket_path.display()
                )));
            }
        },
    };

    let request = stop_request();
    log_debug_message(debug, "daemon control <- ", &request);

    if let Err(error) = write_message(&mut stream, &request) {
        return Err(Error::unexpected(format!(
            "failed to write daemon stop request: {error}"
        )));
    }

    let Some(response) = read_control_message(&stream, CONTROL_TIMEOUT, debug)? else {
        return Err(Error::unexpected(
            "daemon closed the stop control socket without replying",
        ));
    };

    if response_id(&response) != stop_request_id(&request) {
        return Err(Error::unexpected(
            "daemon returned an unexpected stop response id",
        ));
    }
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown daemon stop error");
        return Err(Error::unexpected(message));
    }

    wait_for_stopped_socket(socket_path)?;

    Ok(StopSocketResult::Stopped)
}

fn wait_for_stopped_socket(socket_path: &Path) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < STOP_COMPLETION_TIMEOUT {
        if !socket_path.exists() {
            return Ok(());
        }

        match UnixStream::connect(socket_path) {
            Ok(_) => thread::sleep(POLL_INTERVAL),
            Err(_) => match fs::remove_file(socket_path) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(Error::unexpected(format!(
                        "daemon stopped listening on {} but its socket could not be removed: {error}",
                        socket_path.display()
                    )));
                }
            },
        }
    }

    Err(Error::unexpected(format!(
        "daemon acknowledged stop on {} but did not exit before the timeout",
        socket_path.display()
    )))
}

struct DaemonTarget {
    path: PathBuf,
    workspace_root_string: String,
    root_uri: String,
    workspace_name: String,
    server_name: String,
    socket_path: PathBuf,
}

struct Daemon {
    accept_worker: Option<AcceptWorker>,
    events: EventQueue,
    socket_owned: bool,
    target: DaemonTarget,
    debug: bool,
    idle_timeout: Duration,
    upstream: Option<UpstreamServer>,
    active_client: Option<ClientSession>,
    orphaned_client_requests: BTreeSet<String>,
    idle_since: Instant,
    stop_requested: bool,
}

struct UpstreamServer {
    child: Child,
    stdin: ChildStdin,
    stderr: CapturedStderr,
    generation: u64,
    reader: ReaderWorker,
    initialize_fingerprint: Option<String>,
    initialize_result: Option<Value>,
    restart_required: bool,
    background_work: BackgroundWorkTracker,
}

struct ClientSession {
    writer: UnixStream,
    generation: u64,
    reader: ReaderWorker,
    phase: ClientPhase,
    wants_background_work: bool,
    forwarded_client_requests: BTreeSet<String>,
    pending_server_requests: BTreeMap<String, Value>,
    open_documents: BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum ClientPhase {
    WaitingForInitialize,
    WaitingForInitialized { forward_to_upstream: bool },
    Ready,
    WaitingForExit,
}

impl Daemon {
    fn new(target: DaemonTarget, debug: bool, idle_timeout: Duration) -> Result<Self> {
        let listener = bind_listener(&target.socket_path)?;
        let mut daemon = Self {
            accept_worker: None,
            events: EventQueue::new(),
            socket_owned: true,
            target,
            debug,
            idle_timeout,
            upstream: None,
            active_client: None,
            orphaned_client_requests: BTreeSet::new(),
            idle_since: Instant::now(),
            stop_requested: false,
        };
        daemon.upstream = Some(UpstreamServer::spawn(
            &daemon.target,
            debug,
            &mut daemon.events,
        )?);
        daemon.accept_worker = Some(AcceptWorker::spawn(
            listener,
            &daemon.target.socket_path,
            &daemon.events,
        )?);
        Ok(daemon)
    }

    fn serve(&mut self) -> Result<()> {
        loop {
            if self.stop_requested || self.idle_expired() {
                return self.stop();
            }
            let timeout = if self.active_client.is_none() {
                Some(self.idle_timeout.saturating_sub(self.idle_since.elapsed()))
            } else {
                None
            };
            match self.events.receive(timeout) {
                Ok(event) => {
                    if self.idle_expired() {
                        // A received event may coincide with the deadline. Release its producer
                        // before shutdown starts consuming upstream events from the same queue.
                        let _ = event.acknowledge.send(());
                        return self.stop();
                    }
                    let result = self.dispatch(event.event);
                    // Release admission even when dispatch fails; cancellation handles retired readers.
                    let _ = event.acknowledge.send(());
                    result?;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Error::unexpected(
                        "daemon event workers stopped unexpectedly",
                    ));
                }
            }
        }
    }

    fn idle_expired(&self) -> bool {
        self.active_client.is_none() && self.idle_since.elapsed() >= self.idle_timeout
    }

    fn dispatch(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Accepted(stream) => {
                if self.active_client.is_some() {
                    if handle_busy_connection(stream, self.debug)? {
                        self.stop_requested = true;
                    }
                } else {
                    self.active_client = Some(ClientSession::new(stream, &mut self.events)?);
                }
            }
            Event::AcceptError(error) => {
                return Err(Error::unexpected(format!(
                    "failed to accept client on {}: {error}",
                    self.target.socket_path.display()
                )));
            }
            Event::Reader(Source::Upstream(generation), event)
                if self
                    .upstream
                    .as_ref()
                    .is_some_and(|upstream| upstream.generation == generation) =>
            {
                match event {
                    ReaderEvent::Message(message) => self.handle_upstream_message(&message)?,
                    ReaderEvent::EndOfStream => self.upstream_died(),
                    ReaderEvent::Error(error) => {
                        self.upstream_died();
                        let error = format!("failed to read LSP server message: {error}");
                        log_unexpected_error(&error);
                        return Err(Error::unexpected(error));
                    }
                }
            }
            Event::Reader(Source::Client(generation), event)
                if self
                    .active_client
                    .as_ref()
                    .is_some_and(|client| client.generation == generation) =>
            {
                match event {
                    ReaderEvent::Message(message) => self.handle_client_message(&message)?,
                    ReaderEvent::EndOfStream => self.disconnect_client()?,
                    ReaderEvent::Error(error) => {
                        self.disconnect_client()?;
                        return Err(Error::lsp(format!(
                            "failed to read daemon client message: {error}"
                        )));
                    }
                }
            }
            // Retired workers can still publish a final event while cancellation races with I/O.
            Event::Reader(_, _) => {}
        }
        Ok(())
    }

    fn disconnect_client(&mut self) -> Result<()> {
        let Some(client) = self.active_client.take() else {
            return Ok(());
        };

        client.reader.cancel();

        for uri in client.open_documents {
            let params = DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier::new(parse_lsp_uri(&uri, "document")?),
            };
            let close = jsonrpc::<u64, _>(None, DidCloseTextDocument::METHOD, &params)?;
            let _ = self.write_upstream_message(&close);
        }

        for request_key in client.forwarded_client_requests {
            let id = serde_json::from_value::<NumberOrString>(request_id_from_key(&request_key))
                .map_err(error_fn!(Error::lsp, "invalid cancel request id"))?;
            let params = CancelParams { id };
            let cancel = jsonrpc::<u64, _>(None, Cancel::METHOD, &params)?;
            let _ = self.write_upstream_message(&cancel);
            self.orphaned_client_requests.insert(request_key);
        }

        for request_id in client.pending_server_requests.into_values() {
            let response = error_response(
                &request_id,
                REQUEST_CANCELLED,
                "daemon client disconnected before replying to the LSP server",
            );
            let _ = self.write_upstream_message(&response);
        }

        if !self.stop_requested
            && self
                .upstream
                .as_ref()
                .is_some_and(|upstream| upstream.restart_required)
        {
            self.shutdown_upstream()?;
            self.upstream = Some(UpstreamServer::spawn(
                &self.target,
                self.debug,
                &mut self.events,
            )?);
        }

        self.idle_since = Instant::now();
        Ok(())
    }

    fn write_client_response(&mut self, message: &Value) -> Result<()> {
        log_debug_message(self.debug, "daemon client -> ", message);
        let Some(client) = self.active_client.as_mut() else {
            return Ok(());
        };
        write_message(&mut client.writer, message).map_err(error_fn!(
            Error::lsp,
            "failed to write daemon client message"
        ))
    }

    fn write_upstream_message(&mut self, message: &Value) -> Result<()> {
        let Some(upstream) = self.upstream.as_mut() else {
            return Err(Error::unexpected("LSP server is not running"));
        };
        log_debug_message(self.debug, "daemon upstream <- ", message);
        write_message(&mut upstream.stdin, message)
            .map_err(error_fn!(Error::lsp, "failed to write LSP server message"))
    }

    fn shutdown_upstream(&mut self) -> Result<()> {
        if let Some(mut upstream) = self.upstream.take() {
            upstream.shutdown(self.debug, &mut self.events)?;
        }
        self.orphaned_client_requests.clear();
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.stop_requested = true;
        // Stop accepting before unlinking the socket, so the wakeup still reaches our listener.
        self.accept_worker.take();
        self.disconnect_client()?;
        self.shutdown_upstream()?;
        match fs::remove_file(&self.target.socket_path) {
            Ok(()) => self.socket_owned = false,
            Err(error) if error.kind() == ErrorKind::NotFound => self.socket_owned = false,
            Err(error) => Err(Error::unexpected(format!(
                "failed to remove daemon socket {}: {error}",
                self.target.socket_path.display()
            )))?,
        }
        Ok(())
    }

    fn restart_upstream(&mut self) -> Result<()> {
        self.shutdown_upstream()?;
        self.upstream = Some(UpstreamServer::spawn(
            &self.target,
            self.debug,
            &mut self.events,
        )?);
        Ok(())
    }

    fn upstream_died(&mut self) {
        if let Some(mut upstream) = self.upstream.take() {
            let _ = upstream.stderr.summary();
            match upstream.child.try_wait() {
                Ok(Some(status)) => log_lsp_server_exit(status),
                Ok(None) => {}
                Err(error) => {
                    log_unexpected_error(&format!("failed to inspect LSP server process: {error}"));
                }
            }
        }
        self.upstream = None;
        self.active_client = None;
        self.orphaned_client_requests.clear();
        self.idle_since = Instant::now();
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        // Error exits must also wake acceptance before removing its socket path.
        self.accept_worker.take();
        self.active_client.take();
        self.upstream.take();
        // Normal stop already unlinked our socket; a replacement daemon may own that path now.
        if self.socket_owned {
            let _ = fs::remove_file(&self.target.socket_path);
        }
    }
}
