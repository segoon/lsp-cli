use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use command_group::{CommandGroup as _, GroupChild};
use serde_json::{Value, json};
use wait_timeout::ChildExt as _;

use crate::harness::E2eContext;

const OUTPUT_LIMIT: usize = 16 * 1024;

pub(crate) fn run(
    context: &E2eContext,
    args: &[String],
    workspace: &Path,
    deadline: Duration,
) -> Result<(), String> {
    let mut command = context.command();
    command.args(args);
    let mut session = Session::start(&mut command, workspace, deadline)?;
    session.exchange(|capabilities| context.record_server_capabilities(capabilities))
}

struct Session {
    child: Option<GroupChild>,
    input: Option<ChildStdin>,
    messages: Receiver<Result<Value, String>>,
    stderr: Receiver<Vec<u8>>,
    deadline: Instant,
    workspace_uri: String,
    transcript: Vec<String>,
}

impl Session {
    fn start(command: &mut Command, workspace: &Path, timeout: Duration) -> Result<Self, String> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .group_spawn()
            .map_err(|error| format!("failed to start direct LSP process: {error}"))?;
        let input = child
            .inner()
            .stdin
            .take()
            .ok_or_else(|| "direct LSP process has no stdin pipe".to_string())?;
        let stdout = child
            .inner()
            .stdout
            .take()
            .ok_or_else(|| "direct LSP process has no stdout pipe".to_string())?;
        let stderr = child
            .inner()
            .stderr
            .take()
            .ok_or_else(|| "direct LSP process has no stderr pipe".to_string())?;
        let workspace = workspace
            .canonicalize()
            .map_err(|error| format!("failed to resolve lifecycle workspace: {error}"))?;
        let workspace_uri = url::Url::from_file_path(&workspace)
            .map_err(|()| {
                format!(
                    "failed to convert lifecycle workspace {} to a file URI",
                    workspace.display()
                )
            })?
            .to_string();

        Ok(Self {
            child: Some(child),
            input: Some(input),
            messages: message_reader(stdout),
            stderr: byte_reader(stderr),
            deadline: Instant::now() + timeout,
            workspace_uri,
            transcript: Vec::new(),
        })
    }

    fn exchange(
        &mut self,
        retain_capabilities: impl FnOnce(&Value) -> Result<(), String>,
    ) -> Result<(), String> {
        let initialize = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": self.workspace_uri,
                "capabilities": {},
                "workspaceFolders": [{"uri": self.workspace_uri, "name": "e2e"}]
            }
        });
        self.send(&initialize)?;
        let initialize = self.wait_for_response(1)?;
        let capabilities = initialize
            .pointer("/result/capabilities")
            .cloned()
            .ok_or_else(|| self.diagnostic("initialize response omitted server capabilities"))?;
        retain_capabilities(&capabilities)?;
        self.send(&json!({
            "jsonrpc": "2.0", "method": "initialized", "params": {}
        }))?;
        self.send(&json!({
            "jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null
        }))?;
        self.wait_for_response(2)?;
        self.send(&json!({
            "jsonrpc": "2.0", "method": "exit", "params": null
        }))?;
        // Closing stdin tells servers that ignore `exit` that no more protocol input can arrive.
        drop(self.input.take());
        self.wait_for_exit()
    }

    fn send(&mut self, message: &Value) -> Result<(), String> {
        self.transcript.push(format!("client -> {message}"));
        let body = serde_json::to_vec(message)
            .map_err(|error| self.diagnostic(&format!("failed to encode LSP message: {error}")))?;
        let Some(input) = self.input.as_mut() else {
            return Err(self.diagnostic("direct LSP stdin is closed"));
        };
        write!(input, "Content-Length: {}\r\n\r\n", body.len())
            .and_then(|()| input.write_all(&body))
            .and_then(|()| input.flush())
            .map_err(|error| self.diagnostic(&format!("failed to write LSP message: {error}")))
    }

    fn wait_for_response(&mut self, expected: i64) -> Result<Value, String> {
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            let message = self
                .messages
                .recv_timeout(remaining)
                .map_err(|error| {
                    self.diagnostic(&format!("waiting for LSP response failed: {error}"))
                })?
                .map_err(|error| self.diagnostic(&error))?;
            self.transcript.push(format!("server -> {message}"));
            if message.get("method").is_some() && message.get("id").is_some() {
                self.answer_request(&message)?;
                continue;
            }
            if message.get("id").and_then(Value::as_i64) == Some(expected) {
                if let Some(error) = message.get("error") {
                    return Err(self.diagnostic(&format!(
                        "LSP response {expected} reported an error: {error}"
                    )));
                }
                return Ok(message);
            }
        }
    }

    fn answer_request(&mut self, request: &Value) -> Result<(), String> {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let result = match method {
            "workspace/configuration" => request
                .pointer("/params/items")
                .and_then(Value::as_array)
                .map(|items| Value::Array(vec![Value::Null; items.len()])),
            "workspace/workspaceFolders" => Some(json!([
                {"uri": self.workspace_uri, "name": "e2e"}
            ])),
            "client/registerCapability"
            | "client/unregisterCapability"
            | "window/workDoneProgress/create" => Some(Value::Null),
            _ => None,
        };
        let response = result.map_or_else(
            || {
                json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": {"code": -32601, "message": "unsupported by lifecycle test client"}
                })
            },
            |result| json!({"jsonrpc": "2.0", "id": id, "result": result}),
        );
        self.send(&response)
    }

    fn wait_for_exit(&mut self) -> Result<(), String> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        let result = self
            .child
            .as_mut()
            .expect("direct session should own its child")
            .inner()
            .wait_timeout(remaining)
            .map_err(|error| self.diagnostic(&format!("failed to wait for LSP server: {error}")))?;
        let Some(status) = result else {
            return Err(self.diagnostic("LSP server did not exit before the lifecycle deadline"));
        };
        if status.success() {
            let _reaped_child = self.child.take();
            Ok(())
        } else {
            Err(self.diagnostic(&format!("LSP server exited unsuccessfully: {status}")))
        }
    }

    fn diagnostic(&self, reason: &str) -> String {
        let transcript = excerpt(self.transcript.join("\n").as_bytes());
        format!("{reason}\nLSP transcript:\n{transcript}")
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let failed = self.child.is_some();
        if let Some(mut child) = self.child.take() {
            if let Err(error) = child.kill() {
                eprintln!("failed to kill direct LSP process group: {error}");
            }
            if let Err(error) = child.wait() {
                eprintln!("failed to reap direct LSP process group: {error}");
            }
        }
        if let Ok(stderr) = self.stderr.recv_timeout(Duration::from_secs(1))
            && failed
            && !stderr.is_empty()
        {
            eprintln!("direct LSP stderr:\n{}", excerpt(&stderr));
        }
    }
}

fn message_reader(reader: impl Read + Send + 'static) -> Receiver<Result<Value, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            match read_message(&mut reader) {
                Ok(Some(message)) => {
                    if sender.send(Ok(message)).is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    if sender
                        .send(Err("LSP server closed stdout".to_string()))
                        .is_err()
                    {
                        return;
                    }
                    break;
                }
                Err(error) => {
                    if sender
                        .send(Err(format!("failed to read LSP message: {error}")))
                        .is_err()
                    {
                        return;
                    }
                    break;
                }
            }
        }
    });
    receiver
}

fn byte_reader(mut reader: impl Read + Send + 'static) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if reader.read_to_end(&mut bytes).is_err() {
            bytes.clear();
        }
        let _send_result = sender.send(bytes);
    });
    receiver
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some(value) = header
            .trim_end()
            .strip_prefix("Content-Length:")
            .map(str::trim)
        {
            content_length = Some(value.parse::<usize>().map_err(io::Error::other)?);
        }
    }
    let length = content_length.ok_or_else(|| io::Error::other("missing Content-Length"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(io::Error::other)
}

fn excerpt(bytes: &[u8]) -> String {
    let start = bytes.len().saturating_sub(OUTPUT_LIMIT);
    String::from_utf8_lossy(bytes.get(start..).unwrap_or_default()).into_owned()
}
