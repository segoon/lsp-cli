use std::collections::{BTreeSet, VecDeque};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use lsp_types::WorkspaceFolder;
use lsp_types::notification::{Exit, Initialized, Notification};
use lsp_types::request::{Initialize, Request, Shutdown};
use serde_json::{Value, json};

use super::{
    jsonrpc,
    transport::{log_debug_message, write_message},
};

mod background;
mod process_io;
mod requests;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_initialize_stderr;

use process_io::{CapturedStderr, spawn_reader};

pub struct LspClient {
    transport: ClientTransport,
    stderr: Option<CapturedStderr>,
    messages: Receiver<IncomingMessage>,
    pending_messages: VecDeque<IncomingMessage>,
    next_request_id: u64,
    shutdown_sent: bool,
    opened_documents: BTreeSet<String>,
    workspace_folders: Option<Vec<WorkspaceFolder>>,
    debug: bool,
    timeout: Duration,
}

enum IncomingMessage {
    Message(Value),
    EndOfStream,
    Error(String),
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
    ) -> Result<Self, String> {
        let Some(program) = command.first() else {
            return Err("cannot start LSP server from empty command".to_string());
        };

        let mut child = Command::new(program)
            .args(&command[1..])
            .current_dir(workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if debug {
                Stdio::inherit()
            } else {
                Stdio::piped()
            })
            .spawn()
            .map_err(|error| format_spawn_error(program, &error))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("failed to open stdin for {program}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("failed to open stdout for {program}"))?;
        let stderr = if debug {
            None
        } else {
            Some(CapturedStderr::spawn(child.stderr.take().ok_or_else(
                || format!("failed to open stderr for {program}"),
            )?))
        };
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
            debug,
            timeout,
        })
    }

    pub fn connect_unix(
        socket_path: &Path,
        debug: bool,
        timeout: Duration,
    ) -> Result<Self, String> {
        let stream = UnixStream::connect(socket_path).map_err(|error| {
            format!(
                "failed to connect to daemon socket {}: {error}",
                socket_path.display()
            )
        })?;
        let reader = stream.try_clone().map_err(|error| {
            format!(
                "failed to clone daemon socket {}: {error}",
                socket_path.display()
            )
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
            debug,
            timeout,
        })
    }

    pub fn shutdown(&mut self) -> Result<(), String> {
        if self.shutdown_sent {
            return Ok(());
        }

        let _ = self.send_request::<Shutdown>(&())?;
        self.send_notification::<Exit>(&())?;
        self.shutdown_sent = true;

        self.wait_for_process_exit()?;

        Ok(())
    }

    fn send_request<R>(&mut self, params: &R::Params) -> Result<Value, String>
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
                                return Err(format_lsp_error(R::METHOD, error));
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
                        format!("LSP server closed while waiting for {}", R::METHOD),
                    ));
                }
                Ok(IncomingMessage::Error(error)) => {
                    return Err(format!(
                        "failed to read LSP message for {}: {error}",
                        R::METHOD
                    ));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("timed out waiting for {}", R::METHOD));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(self.format_transport_wait_error(
                        R::METHOD,
                        format!("LSP reader stopped while waiting for {}", R::METHOD),
                    ));
                }
            }
        }
    }

    fn send_notification<N>(&mut self, params: &N::Params) -> Result<(), String>
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

    fn write_transport_message(&mut self, message: &Value) -> Result<(), String> {
        match &mut self.transport {
            ClientTransport::Process { stdin, .. } => write_message(stdin, message),
            ClientTransport::Socket { stream } => write_message(stream, message),
        }
    }

    fn recv_message(&mut self, timeout: Duration) -> Result<IncomingMessage, RecvTimeoutError> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(message);
        }

        self.messages.recv_timeout(timeout)
    }

    fn try_recv_message(&mut self) -> Result<Option<IncomingMessage>, TryRecvError> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(Some(message));
        }

        match self.messages.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn drain_pending_server_requests(&mut self) -> Result<(), String> {
        let mut deferred = VecDeque::new();

        loop {
            match self.try_recv_message() {
                Ok(Some(IncomingMessage::Message(message))) => {
                    if let Some(request_id) = request_id(&message) {
                        self.handle_server_request(&request_id, &message)?;
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
                    deferred.push_back(IncomingMessage::Error(
                        "LSP reader stopped unexpectedly".to_string(),
                    ));
                    break;
                }
            }
        }

        self.pending_messages.extend(deferred);
        Ok(())
    }

    fn format_transport_wait_error(&self, method: &str, error: String) -> String {
        if method != Initialize::METHOD {
            return error;
        }

        let Some(stderr) = self.stderr.as_ref().and_then(CapturedStderr::summary) else {
            return error;
        };

        format!("{error}: {stderr}")
    }

    fn wait_for_process_exit(&mut self) -> Result<(), String> {
        const PROCESS_EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

        let started = Instant::now();
        loop {
            match &mut self.transport {
                ClientTransport::Process { child, .. } => match child.try_wait() {
                    Ok(Some(_)) => return Ok(()),
                    Ok(None) => {}
                    Err(error) => {
                        return Err(format!("failed to wait for LSP server exit: {error}"));
                    }
                },
                ClientTransport::Socket { .. } => return Ok(()),
            }

            let Some(remaining) = self.timeout.checked_sub(started.elapsed()) else {
                self.kill_process()?;
                return Err("timed out waiting for LSP server exit".to_string());
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
                    return Err(format!(
                        "failed to read LSP message while waiting for server exit: {error}"
                    ));
                }
            }
        }
    }

    fn kill_process(&mut self) -> Result<(), String> {
        let ClientTransport::Process { child, .. } = &mut self.transport else {
            return Ok(());
        };

        child
            .kill()
            .map_err(|error| format!("failed to stop LSP server process: {error}"))?;
        child
            .wait()
            .map_err(|error| format!("failed to wait for LSP server exit: {error}"))?;
        Ok(())
    }

    fn handle_server_request(&mut self, request_id: &Value, message: &Value) -> Result<(), String> {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| "server request missing method".to_string())?;
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
}

fn format_spawn_error(program: &str, error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound if !program.contains(std::path::MAIN_SEPARATOR) => {
            format!("LSP server executable `{program}` is not installed or not in $PATH")
        }
        std::io::ErrorKind::NotFound => {
            format!("configured LSP server executable `{program}` was not found")
        }
        _ => format!("failed to start LSP server `{program}`: {error}"),
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if !self.shutdown_sent
            && let ClientTransport::Process { child, .. } = &mut self.transport
        {
            let _ = child.kill();
            let _ = child.wait();
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
