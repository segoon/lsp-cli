use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use lsp_types::notification::{Exit, Initialized, Notification};
use lsp_types::request::{
    CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare,
    DocumentSymbolRequest, GotoDeclaration, GotoDefinition, Initialize, References, Request,
    Shutdown, WorkspaceSymbolRequest,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{InitializeResponse, ServerStatusParams};

pub struct LspClient {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<IncomingMessage>,
    next_request_id: u64,
    shutdown_sent: bool,
    opened_documents: BTreeSet<String>,
    debug: bool,
    timeout: Duration,
}

enum IncomingMessage {
    Message(Value),
    EndOfStream,
    Error(String),
}

#[derive(Debug, Deserialize)]
struct WorkDoneProgressCreateParams {
    token: Value,
}

#[derive(Debug, Deserialize)]
struct ProgressParams {
    token: Value,
    value: ProgressValue,
}

#[derive(Debug, Deserialize)]
struct ProgressValue {
    kind: String,
}

#[derive(Debug, Default)]
struct BuildIndexState {
    saw_server_status: bool,
    saw_work_done_progress: bool,
    active_progress_tokens: std::collections::BTreeSet<String>,
    finished_progress: bool,
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
            opened_documents: BTreeSet::new(),
            debug,
            timeout,
        })
    }

    pub fn open_document(&mut self, path: &Path, uri: &str) -> Result<(), String> {
        if self.opened_documents.contains(uri) {
            return Ok(());
        }

        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        self.send_notification(
            "textDocument/didOpen",
            &json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id(path),
                    "version": 1,
                    "text": text,
                }
            }),
        )?;
        self.opened_documents.insert(uri.to_string());
        Ok(())
    }

    pub fn initialize(
        &mut self,
        root_uri: &str,
        workspace_name: &str,
        want_server_status: bool,
    ) -> Result<InitializeResponse, String> {
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
                "window": {
                    "workDoneProgress": want_server_status,
                },
                "experimental": {
                    "serverStatusNotification": want_server_status,
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

        self.send_notification(Initialized::METHOD, &json!({}))?;
        Ok(response)
    }

    pub fn workspace_symbol(&mut self, pattern: &str) -> Result<Value, String> {
        self.send_request(WorkspaceSymbolRequest::METHOD, &json!({ "query": pattern }))
    }

    pub fn document_symbol(&mut self, uri: &str) -> Result<Value, String> {
        self.send_request(
            DocumentSymbolRequest::METHOD,
            &json!({ "textDocument": { "uri": uri } }),
        )
    }

    pub fn references(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Value, String> {
        self.send_request(
            References::METHOD,
            &json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
                "context": { "includeDeclaration": include_declaration },
            }),
        )
    }

    pub fn definition(&mut self, uri: &str, line: u32, character: u32) -> Result<Value, String> {
        self.send_request(
            GotoDefinition::METHOD,
            &json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
            }),
        )
    }

    pub fn declaration(&mut self, uri: &str, line: u32, character: u32) -> Result<Value, String> {
        self.send_request(
            GotoDeclaration::METHOD,
            &json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
            }),
        )
    }

    pub fn prepare_call_hierarchy(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Value, String> {
        self.send_request(
            CallHierarchyPrepare::METHOD,
            &json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character },
            }),
        )
    }

    pub fn incoming_calls(&mut self, item: &Value) -> Result<Value, String> {
        self.send_request(CallHierarchyIncomingCalls::METHOD, &json!({ "item": item }))
    }

    pub fn outgoing_calls(&mut self, item: &Value) -> Result<Value, String> {
        self.send_request(CallHierarchyOutgoingCalls::METHOD, &json!({ "item": item }))
    }

    pub fn wait_for_background_work(&mut self) -> Result<(), String> {
        let started = Instant::now();
        let mut state = BuildIndexState::default();

        loop {
            let Some(remaining) = self.timeout.checked_sub(started.elapsed()) else {
                return Err(timeout_error(&state));
            };

            match self.messages.recv_timeout(remaining) {
                Ok(IncomingMessage::Message(message)) => {
                    if let Some(outcome) = update_build_index_state(&message, &mut state)? {
                        return outcome;
                    }

                    if let Some(request_id) = request_id(&message) {
                        self.handle_server_request(&request_id, &message)?;
                    }
                }
                Ok(IncomingMessage::EndOfStream) => {
                    return Err(
                        "LSP server closed while waiting for background work to finish".to_string(),
                    );
                }
                Ok(IncomingMessage::Error(error)) => {
                    return Err(format!(
                        "failed to read LSP message while waiting for background work: {error}"
                    ));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(timeout_error(&state));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("LSP reader stopped while waiting for background work".to_string());
                }
            }
        }
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

fn language_id(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("c" | "h") => "c",
        Some("cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx") => "cpp",
        Some("cs") => "csharp",
        Some("go") => "go",
        Some("java") => "java",
        Some("js" | "mjs" | "cjs") => "javascript",
        Some("py") => "python",
        Some("rs") => "rust",
        Some("ts" | "mts" | "cts") => "typescript",
        _ => "plaintext",
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

fn update_build_index_state(
    message: &Value,
    state: &mut BuildIndexState,
) -> Result<Option<Result<(), String>>, String> {
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Ok(None);
    };

    match method {
        "experimental/serverStatus" => {
            state.saw_server_status = true;
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let status: ServerStatusParams = serde_json::from_value(params)
                .map_err(|error| format!("failed to decode experimental/serverStatus: {error}"))?;

            if status.health == "error" {
                return Ok(Some(Err(status.message.unwrap_or_else(|| {
                    "LSP server reported an indexing error".to_string()
                }))));
            }

            if status.quiescent {
                return Ok(Some(Ok(())));
            }
        }
        "window/workDoneProgress/create" => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let create: WorkDoneProgressCreateParams =
                serde_json::from_value(params).map_err(|error| {
                    format!("failed to decode window/workDoneProgress/create: {error}")
                })?;
            state.saw_work_done_progress = true;
            let _ = create.token;
        }
        "$/progress" => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let progress: ProgressParams = serde_json::from_value(params)
                .map_err(|error| format!("failed to decode $/progress: {error}"))?;
            state.saw_work_done_progress = true;
            let token = progress_token(&progress.token);

            match progress.value.kind.as_str() {
                "begin" => {
                    state.active_progress_tokens.insert(token);
                }
                "end" => {
                    state.finished_progress = true;
                    state.active_progress_tokens.remove(&token);
                    if !state.saw_server_status && state.active_progress_tokens.is_empty() {
                        return Ok(Some(Ok(())));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }

    Ok(None)
}

fn progress_token(token: &Value) -> String {
    match token {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}

fn timeout_error(state: &BuildIndexState) -> String {
    if state.saw_server_status || state.saw_work_done_progress {
        "timed out waiting for LSP server to finish background work".to_string()
    } else {
        "selected LSP server did not expose background-work progress".to_string()
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
    serde_json::to_string_pretty(message)
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
            "{\n  \"id\": 1,\n  \"jsonrpc\": \"2.0\"\n}"
        );
    }
}
