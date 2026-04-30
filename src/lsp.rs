use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use lsp_types::notification::{Exit, Initialized, Notification};
use lsp_types::request::{Initialize, Request, Shutdown, WorkspaceSymbolRequest};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

pub struct LspClient {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<IncomingMessage>,
    next_request_id: u64,
    shutdown_sent: bool,
    debug: bool,
    timeout: Duration,
}

enum IncomingMessage {
    Message(Value),
    EndOfStream,
    Error(String),
}

#[derive(Debug, Deserialize)]
pub struct InitializeResponse {
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Deserialize)]
pub struct ServerCapabilities {
    #[serde(rename = "workspaceSymbolProvider")]
    pub workspace_symbol_provider: Option<Value>,
}

impl LspClient {
    pub fn new(command: &[String], debug: bool, timeout: Duration) -> Result<Self, String> {
        let Some(program) = command.first() else {
            return Err("cannot start LSP server from empty command".to_string());
        };

        let mut child = Command::new(program)
            .args(&command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
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
        let messages = spawn_reader(stdout, debug);

        if debug {
            eprintln!("LSP server: {} (pid {})", command.join(" "), child.id());
        }

        Ok(Self {
            child,
            stdin,
            messages,
            next_request_id: 1,
            shutdown_sent: false,
            debug,
            timeout,
        })
    }

    pub fn initialize(&mut self, root_uri: &str, workspace_name: &str) -> Result<(), String> {
        let params = json!({
            "processId": std::process::id(),
            "clientInfo": {
                "name": env!("CARGO_PKG_NAME"),
                "version": env!("CARGO_PKG_VERSION"),
            },
            "rootUri": root_uri,
            "capabilities": {
                "general": {
                    "positionEncodings": ["utf-16"],
                },
            },
            "workspaceFolders": [{
                "uri": root_uri,
                "name": workspace_name,
            }],
        });
        let response = self.send_request(Initialize::METHOD, &params)?;
        let response: InitializeResponse = serde_json::from_value(response)
            .map_err(|error| format!("failed to decode initialize response: {error}"))?;

        if matches!(response.capabilities.workspace_symbol_provider, Some(Value::Bool(false)) | None) {
            return Err("selected LSP server does not support workspace/symbol".to_string());
        }

        self.send_notification(Initialized::METHOD, &json!({}))
    }

    pub fn workspace_symbol(&mut self, pattern: &str) -> Result<Value, String> {
        self.send_request(WorkspaceSymbolRequest::METHOD, &json!({ "query": pattern }))
    }

    pub fn shutdown(&mut self) -> Result<(), String> {
        if self.shutdown_sent {
            return Ok(());
        }

        let _ = self.send_request(Shutdown::METHOD, &Value::Null)?;
        self.send_notification(Exit::METHOD, &Value::Null)?;
        self.shutdown_sent = true;

        self.child
            .wait()
            .map_err(|error| format!("failed to wait for LSP server exit: {error}"))?;

        Ok(())
    }

    fn send_request(&mut self, method: &str, params: &Value) -> Result<Value, String> {
        let id = self.next_request_id;
        self.next_request_id += 1;

        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        log_debug_message(self.debug, "-> ", &message);
        write_message(&mut self.stdin, &message)?;

        loop {
            match self.messages.recv_timeout(self.timeout) {
                Ok(IncomingMessage::Message(message)) => {
                    if let Some(response_id) = response_id(&message) {
                        if response_id == id {
                            if let Some(error) = message.get("error") {
                                return Err(format_lsp_error(method, error));
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
                    return Err(format!("LSP server closed while waiting for {method}"));
                }
                Ok(IncomingMessage::Error(error)) => {
                    return Err(format!("failed to read LSP message for {method}: {error}"));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("timed out waiting for {method}"));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(format!("LSP reader stopped while waiting for {method}"));
                }
            }
        }
    }

    fn send_notification(&mut self, method: &str, params: &Value) -> Result<(), String> {
        let message = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        log_debug_message(self.debug, "-> ", &message);
        write_message(&mut self.stdin, &message)
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
            "workspace/configuration" | "workspace/workspaceFolders" => json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": [],
            }),
            "client/registerCapability" | "client/unregisterCapability" | "window/workDoneProgress/create" => {
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
        write_message(&mut self.stdin, &response)
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
        if !self.shutdown_sent {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn spawn_reader(stdout: ChildStdout, debug: bool) -> Receiver<IncomingMessage> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || reader_loop(stdout, &sender, debug));
    receiver
}

fn reader_loop(stdout: ChildStdout, sender: &Sender<IncomingMessage>, debug: bool) {
    let mut reader = BufReader::new(stdout);

    loop {
        match read_message(&mut reader) {
            Ok(Some(message)) => {
                log_debug_message(debug, "<- ", &message);
                if sender.send(IncomingMessage::Message(message)).is_err() {
                    return;
                }
            }
            Ok(None) => {
                let _ = sender.send(IncomingMessage::EndOfStream);
                return;
            }
            Err(error) => {
                let _ = sender.send(IncomingMessage::Error(error));
                return;
            }
        }
    }
}

fn read_message<R>(reader: &mut BufReader<R>) -> Result<Option<Value>, String>
where
    R: Read,
{
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| format!("failed to read LSP header: {error}"))?;

        if bytes == 0 {
            return if content_length.is_none() {
                Ok(None)
            } else {
                Err("unexpected EOF while reading LSP headers".to_string())
            };
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        let Some((name, value)) = trimmed.split_once(':') else {
            return Err(format!("invalid LSP header: {trimmed}"));
        };

        if name.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| format!("invalid Content-Length {value:?}: {error}"))?,
            );
        }
    }

    let Some(content_length) = content_length else {
        return Err("missing Content-Length header".to_string());
    };

    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|error| format!("failed to read LSP body: {error}"))?;
    serde_json::from_slice(&body).map_err(|error| format!("invalid JSON-RPC payload: {error}"))
}

fn write_message<W>(writer: &mut W, message: &Value) -> Result<(), String>
where
    W: Write,
{
    let body = serde_json::to_vec(message)
        .map_err(|error| format!("failed to serialize JSON-RPC message: {error}"))?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .and_then(|()| writer.write_all(&body))
        .and_then(|()| writer.flush())
        .map_err(|error| format!("failed to write JSON-RPC message: {error}"))
}

fn log_debug_message(debug: bool, prefix: &str, message: &Value) {
    if debug {
        eprintln!("{prefix}{}", serialize_debug_message(message));
    }
}

fn serialize_debug_message(message: &Value) -> String {
    serde_json::to_string(message)
        .unwrap_or_else(|_| "<failed to serialize debug message>".to_string())
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

pub fn workspace_name(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map_or_else(|| path.display().to_string(), ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::{format_spawn_error, read_message, serialize_debug_message, write_message};
    use serde_json::json;
    use std::io::BufReader;

    #[test]
    fn writes_and_reads_lsp_message() {
        let mut buffer = Vec::new();
        let message = json!({"jsonrpc": "2.0", "id": 1, "result": null});
        write_message(&mut buffer, &message).expect("message should be written");

        let mut reader = BufReader::new(buffer.as_slice());
        assert_eq!(
            read_message(&mut reader).expect("message should read"),
            Some(message)
        );
    }

    #[test]
    fn formats_missing_binary_error() {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");

        assert_eq!(
            format_spawn_error("ast-grep", &error),
            "LSP server executable `ast-grep` is not installed or not in $PATH"
        );
    }

    #[test]
    fn serializes_debug_messages_as_json() {
        assert_eq!(
            serialize_debug_message(&json!({"jsonrpc": "2.0", "id": 1})),
            "{\"id\":1,\"jsonrpc\":\"2.0\"}"
        );
    }
}
