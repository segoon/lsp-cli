use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use lsp_types::WorkspaceFolder;
use lsp_types::notification::{Exit, Initialized, Notification, PublishDiagnostics};
use lsp_types::request::{Initialize, Request, Shutdown};
use serde_json::{Value, json};

use super::{
    jsonrpc,
    transport::{log_debug_message, write_message},
};
use crate::error::{Error, Result};
use crate::server_stderr::CapturedStderr;
use crate::system_log::{
    log_lsp_server_cmdline, log_lsp_server_cwd, log_lsp_server_exit, log_lsp_server_started,
    log_lsp_server_starting, log_unexpected_error,
};

mod background;
mod process_io;
mod requests;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_initialize_stderr;
#[cfg(test)]
mod tests_logging;

use process_io::spawn_reader;

pub struct LspClient {
    transport: ClientTransport,
    stderr: Option<CapturedStderr>,
    messages: Receiver<IncomingMessage>,
    pending_messages: VecDeque<IncomingMessage>,
    next_request_id: u64,
    shutdown_sent: bool,
    opened_documents: BTreeSet<String>,
    workspace_folders: Option<Vec<WorkspaceFolder>>,
    published_diagnostics: BTreeMap<String, Value>,
    process_exit_logged: bool,
    debug: bool,
    timeout: Duration,
}

enum IncomingMessage {
    Message(Value),
    EndOfStream,
    Error(Error),
}

enum ClientTransport {
    Process { child: Child, stdin: ChildStdin },
    Socket { stream: UnixStream },
}

impl LspClient {
    pub fn new(
        command: &[String],
        workspace_root: &Path,
        debug: bool,
        timeout: Duration,
    ) -> Result<Self> {
        let Some(program) = command.first() else {
            return Err(Error::unexpected(
                "cannot start LSP server from empty command",
            ));
        };

        log_lsp_server_starting();
        log_lsp_server_cmdline(command);
        log_lsp_server_cwd(workspace_root);
        let mut child = Command::new(program)
            .args(&command[1..])
            .current_dir(workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                let error = format_spawn_error(program, &error);
                if error.should_log_as_unexpected() {
                    log_unexpected_error(&error.to_string());
                }
                error
            })?;

        log_lsp_server_started(child.id());

        let stdin = child.stdin.take().ok_or_else(|| {
            let error = Error::unexpected(format!("failed to open stdin for {program}"));
            log_unexpected_error(&error.to_string());
            error
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            let error = Error::unexpected(format!("failed to open stdout for {program}"));
            log_unexpected_error(&error.to_string());
            error
        })?;
        let stderr = Some(CapturedStderr::spawn(
            child.stderr.take().ok_or_else(|| {
                let error = Error::unexpected(format!("failed to open stderr for {program}"));
                log_unexpected_error(&error.to_string());
                error
            })?,
            debug,
        ));
        let messages = spawn_reader(stdout, debug);

        if debug {
            eprintln!("LSP server: {} (pid {})", command.join(" "), child.id());
        }

        Ok(Self {
            transport: ClientTransport::Process { child, stdin },
            stderr,
            messages,
            pending_messages: VecDeque::new(),
            next_request_id: 1,
            shutdown_sent: false,
            opened_documents: BTreeSet::new(),
            workspace_folders: None,
            published_diagnostics: BTreeMap::new(),
            process_exit_logged: false,
            debug,
            timeout,
        })
    }

    pub fn connect_unix(socket_path: &Path, debug: bool, timeout: Duration) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).map_err(|error| {
            Error::lsp(format!(
                "failed to connect to daemon socket {}: {error}",
                socket_path.display()
            ))
        })?;
        let reader = stream.try_clone().map_err(|error| {
            Error::lsp(format!(
                "failed to clone daemon socket {}: {error}",
                socket_path.display()
            ))
        })?;
        let messages = spawn_reader(reader, debug);

        if debug {
            eprintln!("LSP daemon socket: {}", socket_path.display());
        }

        Ok(Self {
            transport: ClientTransport::Socket { stream },
            stderr: None,
            messages,
            pending_messages: VecDeque::new(),
            next_request_id: 1,
            shutdown_sent: false,
            opened_documents: BTreeSet::new(),
            workspace_folders: None,
            published_diagnostics: BTreeMap::new(),
            process_exit_logged: true,
            debug,
            timeout,
        })
    }

    pub fn shutdown(&mut self) -> Result<()> {
        if self.shutdown_sent {
            return Ok(());
        }

        let _ = self.send_request::<Shutdown>(&())?;
        self.send_notification::<Exit>(&())?;
        self.shutdown_sent = true;

        self.wait_for_process_exit()?;

        Ok(())
    }

    fn send_request<R>(&mut self, params: &R::Params) -> Result<Value>
    where
        R: Request,
    {
        if R::METHOD != Initialize::METHOD {
            self.drain_pending_server_requests()?;
        }

        let id = self.next_request_id;
        self.next_request_id += 1;

        let message = jsonrpc(Some(id), R::METHOD, params)?;
        log_debug_message(self.debug, "-> ", &message);
        self.write_transport_message(&message)?;

        loop {
            match self.recv_message(self.timeout) {
                Ok(IncomingMessage::Message(message)) => {
                    if let Some(response_id) = response_id(&message) {
                        if response_id == id {
                            if let Some(error) = message.get("error") {
                                return Err(Error::lsp(format_lsp_error(R::METHOD, error)));
                            }

                            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
                        }

                        continue;
                    }

                    if let Some(request_id) = request_id(&message) {
                        self.handle_server_request(&request_id, &message)?;
                    }
                }
                Ok(IncomingMessage::EndOfStream) => {
                    return Err(self.format_transport_wait_error(
                        R::METHOD,
                        Error::lsp(format!("LSP server closed while waiting for {}", R::METHOD)),
                    ));
                }
                Ok(IncomingMessage::Error(error)) => {
                    let error =
                        error.with_prefix(format!("failed to read LSP message for {}", R::METHOD));
                    if error.should_log_as_unexpected() {
                        log_unexpected_error(&error.to_string());
                    }
                    return Err(error);
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(Error::lsp(format!("timed out waiting for {}", R::METHOD)));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(self.format_transport_wait_error(
                        R::METHOD,
                        Error::lsp(format!(
                            "LSP reader stopped while waiting for {}",
                            R::METHOD
                        )),
                    ));
                }
            }
        }
    }

    fn send_notification<N>(&mut self, params: &N::Params) -> Result<()>
    where
        N: Notification,
    {
        if N::METHOD != Initialized::METHOD {
            self.drain_pending_server_requests()?;
        }

        let message = jsonrpc::<u64, _>(None, N::METHOD, params)?;
        log_debug_message(self.debug, "-> ", &message);
        self.write_transport_message(&message)
    }

    fn write_transport_message(&mut self, message: &Value) -> Result<()> {
        match &mut self.transport {
            ClientTransport::Process { stdin, .. } => write_message(stdin, message),
            ClientTransport::Socket { stream } => write_message(stream, message),
        }
    }

    fn recv_message(
        &mut self,
        timeout: Duration,
    ) -> std::result::Result<IncomingMessage, RecvTimeoutError> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(message);
        }

        self.messages.recv_timeout(timeout)
    }

    fn try_recv_message(&mut self) -> std::result::Result<Option<IncomingMessage>, TryRecvError> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(Some(message));
        }

        match self.messages.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn drain_pending_server_requests(&mut self) -> Result<()> {
        let mut deferred = VecDeque::new();

        loop {
            match self.try_recv_message() {
                Ok(Some(IncomingMessage::Message(message))) => {
                    if let Some(request_id) = request_id(&message) {
                        self.handle_server_request(&request_id, &message)?;
                    } else if self.handle_server_notification(&message)? {
                    } else {
                        deferred.push_back(IncomingMessage::Message(message));
                    }
                }
                Ok(Some(message @ (IncomingMessage::EndOfStream | IncomingMessage::Error(_)))) => {
                    deferred.push_back(message);
                    break;
                }
                Ok(None) | Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    deferred.push_back(IncomingMessage::Error(Error::lsp(
                        "LSP reader stopped unexpectedly",
                    )));
                    break;
                }
            }
        }

        self.pending_messages.extend(deferred);
        Ok(())
    }

    pub fn take_published_diagnostics(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.published_diagnostics)
            .into_values()
            .collect()
    }

    pub fn published_diagnostics_len(&self) -> usize {
        self.published_diagnostics.len()
    }

    pub fn collect_diagnostics(&mut self, timeout: Duration) -> Result<()> {
        loop {
            match self.recv_message(timeout) {
                Ok(IncomingMessage::Message(message)) => {
                    if let Some(request_id) = request_id(&message) {
                        self.handle_server_request(&request_id, &message)?;
                        continue;
                    }
                    if self.handle_server_notification(&message)? {
                        continue;
                    }
                    self.pending_messages
                        .push_back(IncomingMessage::Message(message));
                    return Ok(());
                }
                Ok(IncomingMessage::EndOfStream) | Err(RecvTimeoutError::Timeout) => return Ok(()),
                Ok(IncomingMessage::Error(error)) => {
                    let error = error.with_prefix("failed to read LSP diagnostics notification");
                    if error.should_log_as_unexpected() {
                        log_unexpected_error(&error.to_string());
                    }
                    return Err(error);
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Ok(());
                }
            }
        }
    }

    pub fn drain_server_notifications(&mut self) -> Result<()> {
        self.drain_pending_server_requests()
    }

    fn format_transport_wait_error(&mut self, method: &str, error: Error) -> Error {
        self.try_log_process_exit();

        if method != Initialize::METHOD {
            return error;
        }

        let Some(stderr) = self.stderr.as_ref().and_then(CapturedStderr::summary) else {
            return error;
        };

        error.with_prefix(stderr)
    }

    fn try_log_process_exit(&mut self) {
        let ClientTransport::Process { child, .. } = &mut self.transport else {
            return;
        };
        let Ok(Some(status)) = child.try_wait() else {
            return;
        };
        self.log_process_exit(status);
    }

    fn wait_for_process_exit(&mut self) -> Result<()> {
        const PROCESS_EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

        let started = Instant::now();
        loop {
            match &mut self.transport {
                ClientTransport::Process { child, .. } => match child.try_wait() {
                    Ok(Some(status)) => {
                        self.log_process_exit(status);
                        return Ok(());
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let error = Error::unexpected(format!(
                            "failed to wait for LSP server exit: {error}"
                        ));
                        log_unexpected_error(&error.to_string());
                        return Err(error);
                    }
                },
                ClientTransport::Socket { .. } => return Ok(()),
            }

            let Some(remaining) = self.timeout.checked_sub(started.elapsed()) else {
                self.kill_process()?;
                return Err(Error::unexpected("timed out waiting for LSP server exit"));
            };
            let poll_timeout = if remaining < PROCESS_EXIT_POLL_INTERVAL {
                remaining
            } else {
                PROCESS_EXIT_POLL_INTERVAL
            };

            match self.recv_message(poll_timeout) {
                Ok(IncomingMessage::Message(message)) => {
                    if let Some(request_id) = request_id(&message) {
                        self.handle_server_request(&request_id, &message)?;
                    }
                }
                Ok(IncomingMessage::EndOfStream)
                | Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
                Ok(IncomingMessage::Error(error)) => {
                    let error = error
                        .with_prefix("failed to read LSP message while waiting for server exit");
                    if error.should_log_as_unexpected() {
                        log_unexpected_error(&error.to_string());
                    }
                    return Err(error);
                }
            }
        }
    }

    fn kill_process(&mut self) -> Result<()> {
        let ClientTransport::Process { child, .. } = &mut self.transport else {
            return Ok(());
        };

        child.kill().map_err(|error| {
            let error = Error::unexpected(format!("failed to stop LSP server process: {error}"));
            log_unexpected_error(&error.to_string());
            error
        })?;
        let status = child.wait().map_err(|error| {
            let error = Error::unexpected(format!("failed to wait for LSP server exit: {error}"));
            log_unexpected_error(&error.to_string());
            error
        })?;
        self.log_process_exit(status);
        Ok(())
    }

    fn log_process_exit(&mut self, status: std::process::ExitStatus) {
        if self.process_exit_logged {
            return;
        }
        log_lsp_server_exit(status);
        self.process_exit_logged = true;
    }

    fn handle_server_request(&mut self, request_id: &Value, message: &Value) -> Result<()> {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::lsp("server request missing method"))?;
        let response = match method {
            "window/showMessageRequest" => json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": Value::Null,
            }),
            "workspace/configuration" => json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": [],
            }),
            "workspace/workspaceFolders" => json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": serde_json::to_value(self.workspace_folders.clone())
                    .expect("workspace folders should serialize"),
            }),
            "client/registerCapability"
            | "client/unregisterCapability"
            | "window/workDoneProgress/create" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": Value::Null,
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {
                    "code": -32601,
                    "message": format!("unsupported client request: {method}"),
                },
            }),
        };

        log_debug_message(self.debug, "-> ", &response);
        self.write_transport_message(&response)
    }

    fn handle_server_notification(&mut self, message: &Value) -> Result<bool> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Ok(false);
        };

        if method != PublishDiagnostics::METHOD {
            return Ok(false);
        }

        let Some(params) = message.get("params") else {
            return Err(Error::lsp("publishDiagnostics notification missing params"));
        };
        let Some(uri) = params.get("uri").and_then(Value::as_str) else {
            return Err(Error::lsp("publishDiagnostics notification missing uri"));
        };

        self.published_diagnostics
            .insert(uri.to_string(), message.clone());
        Ok(true)
    }
}

fn format_spawn_error(program: &str, error: &std::io::Error) -> Error {
    match error.kind() {
        std::io::ErrorKind::NotFound if !program.contains(std::path::MAIN_SEPARATOR) => {
            Error::missing_executable(format!(
                "LSP server executable `{program}` is not installed or not in $PATH"
            ))
        }
        std::io::ErrorKind::NotFound => Error::missing_executable(format!(
            "configured LSP server executable `{program}` was not found"
        )),
        _ => Error::unexpected(format!("failed to start LSP server `{program}`: {error}")),
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if !self.shutdown_sent
            && let ClientTransport::Process { child, .. } = &mut self.transport
        {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !self.process_exit_logged {
                        log_lsp_server_exit(status);
                        self.process_exit_logged = true;
                    }
                    return;
                }
                Ok(None) => {}
                Err(error) => {
                    log_unexpected_error(&format!("failed to inspect LSP server process: {error}"));
                    return;
                }
            }

            if let Err(error) = child.kill() {
                log_unexpected_error(&format!("failed to stop LSP server process: {error}"));
                return;
            }
            match child.wait() {
                Ok(status) if !self.process_exit_logged => {
                    log_lsp_server_exit(status);
                    self.process_exit_logged = true;
                }
                Ok(_) => {}
                Err(error) => {
                    log_unexpected_error(&format!("failed to wait for LSP server exit: {error}"));
                }
            }
        }
    }
}

fn response_id(message: &Value) -> Option<u64> {
    message
        .get("id")
        .and_then(Value::as_u64)
        .filter(|_| message.get("method").is_none())
}

fn request_id(message: &Value) -> Option<Value> {
    message
        .get("id")
        .filter(|_| message.get("method").is_some())
        .cloned()
}

fn format_lsp_error(method: &str, error: &Value) -> String {
    let code = error
        .get("code")
        .and_then(Value::as_i64)
        .map_or_else(|| "unknown".to_string(), |value| value.to_string());
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown error");

    format!("{method} failed with {code}: {message}")
}
