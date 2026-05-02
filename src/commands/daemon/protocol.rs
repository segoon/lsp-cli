use super::{BUSY_CLIENT_TIMEOUT, Daemon, DaemonTarget, INVALID_REQUEST, REQUEST_CANCELLED};
use crate::lsp::ServerStatusParams;
use crate::lsp::transport::{log_debug_message, read_message, write_message};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub(super) const STOP_METHOD: &str = "$/lsp-cli/stop";

pub(super) enum ReaderEvent {
    Message(Value),
    EndOfStream,
    Error(String),
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum BackgroundWorkState {
    #[default]
    Unknown,
    InProgress,
    Quiescent,
}

#[derive(Debug, Default)]
pub(super) struct BackgroundWorkTracker {
    pub(super) state: BackgroundWorkState,
    pub(super) active_progress_tokens: BTreeSet<String>,
}

impl BackgroundWorkTracker {
    pub(super) fn is_quiescent(&self) -> bool {
        self.state == BackgroundWorkState::Quiescent
    }
}

impl Daemon {
    pub(super) fn notify_client_if_background_ready(
        &mut self,
        reused_initialize: bool,
    ) -> Result<(), String> {
        if !reused_initialize {
            return Ok(());
        }

        let should_notify = self
            .active_client
            .as_ref()
            .is_some_and(|client| client.wants_background_work)
            && self
                .upstream
                .as_ref()
                .is_some_and(|upstream| upstream.background_work.is_quiescent());
        if !should_notify {
            return Ok(());
        }

        // Reused daemon sessions can attach after the upstream server already finished indexing.
        // Emit a synthetic quiescent notification so wait_for_background_work sees the warm state.
        self.write_client_response(&background_work_ready_notification())
    }
}

pub(super) fn handle_busy_connection(
    mut stream: UnixStream,
    debug: bool,
) -> Result<bool, String> {
    let _ = stream.set_read_timeout(Some(BUSY_CLIENT_TIMEOUT));
    let Ok(reader_stream) = stream.try_clone() else {
        return Ok(false);
    };
    let mut reader = BufReader::new(reader_stream);
    let Ok(Some(message)) = read_message(&mut reader) else {
        return Ok(false);
    };
    log_debug_message(debug, "daemon busy client <- ", &message);

    if stop_request_id(&message).is_some() {
        respond_to_stop_request(&mut stream, &message, debug)?;
        return Ok(true);
    }

    let Some(request_id) = request_id(&message) else {
        return Ok(false);
    };
    if message_method(&message) != Some("initialize") {
        return Ok(false);
    }

    let response = error_response(
        &request_id,
        REQUEST_CANCELLED,
        "another daemon client is already connected",
    );
    let _ = write_message(&mut stream, &response);
    Ok(false)
}

pub(super) fn read_control_message(
    stream: &UnixStream,
    timeout: Duration,
    debug: bool,
) -> Result<Option<Value>, String> {
    let _ = stream.set_read_timeout(Some(timeout));
    let reader = stream
        .try_clone()
        .map_err(|error| format!("failed to clone daemon control socket: {error}"))?;
    let mut reader = BufReader::new(reader);
    let message = read_message(&mut reader)?;
    if let Some(message) = &message {
        log_debug_message(debug, "daemon control -> ", message);
    }
    Ok(message)
}

pub(super) fn stop_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "lsp-cli/stop",
        "method": STOP_METHOD,
        "params": Value::Null,
    })
}

pub(super) fn stop_request_id(message: &Value) -> Option<Value> {
    if message_method(message) == Some(STOP_METHOD) {
        request_id(message)
    } else {
        None
    }
}

pub(super) fn respond_to_stop_request(
    stream: &mut UnixStream,
    message: &Value,
    debug: bool,
) -> Result<(), String> {
    let Some(request_id) = stop_request_id(message) else {
        return Err("daemon stop request is missing an id".to_string());
    };
    let response = success_response(&request_id, &Value::Null);
    log_debug_message(debug, "daemon control <- ", &response);
    write_message(stream, &response)
        .map_err(|error| format!("failed to write daemon stop response: {error}"))
}

pub(super) fn local_server_request_response(request_id: &Value, method: &str) -> Value {
    match method {
        "window/showMessageRequest"
        | "client/registerCapability"
        | "client/unregisterCapability"
        | "window/workDoneProgress/create" => success_response(request_id, &Value::Null),
        "workspace/configuration" | "workspace/workspaceFolders" => {
            success_response(request_id, &json!([]))
        }
        "workspace/applyEdit" => json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": {
                "applied": false,
                "failureReason": "no daemon client is connected to apply workspace edits",
            }
        }),
        _ => error_response(
            request_id,
            INVALID_REQUEST,
            &format!("daemon does not support server request {method}"),
        ),
    }
}

pub(super) fn update_background_work_tracker(
    message: &Value,
    tracker: &mut BackgroundWorkTracker,
) -> Result<(), String> {
    let Some(method) = message_method(message) else {
        return Ok(());
    };

    match method {
        "experimental/serverStatus" => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let status: ServerStatusParams = serde_json::from_value(params)
                .map_err(|error| format!("failed to decode experimental/serverStatus: {error}"))?;
            tracker.state = if status.quiescent {
                BackgroundWorkState::Quiescent
            } else {
                BackgroundWorkState::InProgress
            };
            if status.quiescent {
                tracker.active_progress_tokens.clear();
            }
        }
        "$/progress" => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let progress: ProgressParams = serde_json::from_value(params)
                .map_err(|error| format!("failed to decode $/progress: {error}"))?;
            let token = progress_token(&progress.token);

            match progress.value.kind.as_str() {
                "begin" => {
                    tracker.state = BackgroundWorkState::InProgress;
                    tracker.active_progress_tokens.insert(token);
                }
                "end" => {
                    tracker.active_progress_tokens.remove(&token);
                    if tracker.active_progress_tokens.is_empty() {
                        tracker.state = BackgroundWorkState::Quiescent;
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }

    Ok(())
}

fn progress_token(token: &Value) -> String {
    match token {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}

pub(super) fn wants_background_work(params: &Value) -> bool {
    params
        .get("capabilities")
        .and_then(Value::as_object)
        .is_some_and(|capabilities| {
            capabilities
                .get("window")
                .and_then(Value::as_object)
                .and_then(|window| window.get("workDoneProgress"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || capabilities
                    .get("experimental")
                    .and_then(Value::as_object)
                    .and_then(|experimental| experimental.get("serverStatusNotification"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
}

pub(super) fn background_work_ready_notification() -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "experimental/serverStatus",
        "params": {
            "health": "ok",
            "quiescent": true,
            "message": Value::Null,
        }
    })
}

pub(super) fn normalize_initialize_params(
    params: &Value,
    target: &DaemonTarget,
) -> Result<Value, String> {
    let Some(object) = params.as_object() else {
        return Err("initialize params must be a JSON object".to_string());
    };

    if let Some(root_uri) = object.get("rootUri")
        && !root_uri.is_null()
        && root_uri.as_str() != Some(target.root_uri.as_str())
    {
        return Err(format!(
            "daemon client rootUri must match {}",
            target.root_uri
        ));
    }

    if let Some(root_path) = object.get("rootPath")
        && !root_path.is_null()
        && root_path.as_str() != Some(target.workspace_root_string.as_str())
    {
        return Err(format!(
            "daemon client rootPath must match {}",
            target.workspace_root_string
        ));
    }

    if let Some(workspace_folders) = object.get("workspaceFolders")
        && !workspace_folders.is_null()
    {
        let Some(items) = workspace_folders.as_array() else {
            return Err("initialize workspaceFolders must be an array or null".to_string());
        };
        if items.len() != 1
            || items[0].get("uri").and_then(Value::as_str) != Some(target.root_uri.as_str())
        {
            return Err(format!(
                "daemon client workspaceFolders must contain only {}",
                target.root_uri
            ));
        }
    }

    let mut normalized = object.clone();
    normalized.insert(
        "processId".to_string(),
        Value::from(u64::from(std::process::id())),
    );
    normalized.remove("workDoneToken");
    normalized.insert(
        "rootUri".to_string(),
        Value::String(target.root_uri.clone()),
    );
    normalized.insert(
        "rootPath".to_string(),
        Value::String(target.workspace_root_string.clone()),
    );
    normalized.insert(
        "workspaceFolders".to_string(),
        json!([{
            "uri": target.root_uri,
            "name": target.workspace_name,
        }]),
    );

    Ok(Value::Object(normalized))
}

pub(super) fn request_id(message: &Value) -> Option<Value> {
    message
        .get("id")
        .filter(|_| message.get("method").is_some())
        .cloned()
}

pub(super) fn response_id(message: &Value) -> Option<Value> {
    message
        .get("id")
        .filter(|_| message.get("method").is_none())
        .cloned()
}

pub(super) fn message_method(message: &Value) -> Option<&str> {
    message.get("method").and_then(Value::as_str)
}

pub(super) fn success_response(id: &Value, result: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

pub(super) fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

pub(super) fn fingerprint_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(string) => {
            serde_json::to_string(string).unwrap_or_else(|_| "\"<invalid-string>\"".to_string())
        }
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(fingerprint_value)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(object) => {
            let mut pairs = object.iter().collect::<Vec<_>>();
            pairs.sort_by(|(left, _), (right, _)| left.cmp(right));
            format!(
                "{{{}}}",
                pairs
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key)
                            .unwrap_or_else(|_| "\"<invalid-key>\"".to_string()),
                        fingerprint_value(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

pub(super) fn id_key(id: &Value) -> String {
    fingerprint_value(id)
}

pub(super) fn request_id_from_key(key: &str) -> Value {
    serde_json::from_str(key).unwrap_or_else(|_| Value::String(key.to_string()))
}
