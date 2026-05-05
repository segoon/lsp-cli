use super::{BUSY_CLIENT_TIMEOUT, Daemon, DaemonTarget, INVALID_REQUEST, REQUEST_CANCELLED};
use crate::error::{Error, Result, error_fn};
use crate::lsp::transport::{log_debug_message, read_message, write_message};
use crate::lsp::{SERVER_STATUS_METHOD, ServerStatusParams, StopParams, jsonrpc, parse_lsp_uri};
use lsp_types::{ApplyWorkspaceEditResponse, WorkspaceFolder};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub(super) use crate::lsp::STOP_METHOD;

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
    ) -> Result<()> {
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

pub(super) fn handle_busy_connection(mut stream: UnixStream, debug: bool) -> Result<bool> {
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
) -> Result<Option<Value>> {
    let _ = stream.set_read_timeout(Some(timeout));
    let reader = stream.try_clone().map_err(error_fn!(
        Error::unexpected,
        "failed to clone daemon control socket"
    ))?;
    let mut reader = BufReader::new(reader);
    let message = read_message(&mut reader)?;
    if let Some(message) = &message {
        log_debug_message(debug, "daemon control -> ", message);
    }
    Ok(message)
}

pub(super) fn stop_request() -> Value {
    jsonrpc(Some("lsp-cli/stop"), STOP_METHOD, &StopParams).expect("stop request should encode")
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
) -> Result<()> {
    let Some(request_id) = stop_request_id(message) else {
        return Err(Error::lsp("daemon stop request is missing an id"));
    };
    let response = success_response(&request_id, &Value::Null);
    log_debug_message(debug, "daemon control <- ", &response);
    write_message(stream, &response).map_err(error_fn!(
        Error::lsp,
        "failed to write daemon stop response"
    ))
}

pub(super) fn local_server_request_response(request_id: &Value, method: &str) -> Value {
    match method {
        "window/showMessageRequest"
        | "client/registerCapability"
        | "client/unregisterCapability"
        | "window/workDoneProgress/create" => success_response(request_id, &Value::Null),
        "workspace/configuration" => success_response(request_id, &json!([])),
        "workspace/workspaceFolders" => success_response(
            request_id,
            &serde_json::to_value(Vec::<WorkspaceFolder>::new())
                .expect("workspace folders should serialize"),
        ),
        "workspace/applyEdit" => json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": serde_json::to_value(ApplyWorkspaceEditResponse {
                applied: false,
                failure_reason: Some(
                    "no daemon client is connected to apply workspace edits".to_string(),
                ),
                failed_change: None,
            })
            .expect("applyEdit response should serialize")
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
) -> Result<()> {
    let Some(method) = message_method(message) else {
        return Ok(());
    };

    match method {
        SERVER_STATUS_METHOD => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let status: ServerStatusParams = serde_json::from_value(params).map_err(error_fn!(
                Error::lsp,
                "failed to decode {}",
                SERVER_STATUS_METHOD
            ))?;
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
                .map_err(error_fn!(Error::lsp, "failed to decode $/progress"))?;
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
    let params = ServerStatusParams {
        health: "ok".to_string(),
        quiescent: true,
        message: None,
    };
    jsonrpc::<u64, _>(None, SERVER_STATUS_METHOD, &params)
        .expect("server status notification should encode")
}

pub(super) fn normalize_initialize_params(params: &Value, target: &DaemonTarget) -> Result<Value> {
    let Some(object) = params.as_object() else {
        return Err(Error::lsp("initialize params must be a JSON object"));
    };

    if let Some(root_uri) = object.get("rootUri")
        && !root_uri.is_null()
        && root_uri.as_str() != Some(target.root_uri.as_str())
    {
        return Err(Error::lsp(format!(
            "daemon client rootUri must match {}",
            target.root_uri
        )));
    }

    if let Some(root_path) = object.get("rootPath")
        && !root_path.is_null()
        && root_path.as_str() != Some(target.workspace_root_string.as_str())
    {
        return Err(Error::lsp(format!(
            "daemon client rootPath must match {}",
            target.workspace_root_string
        )));
    }

    if let Some(workspace_folders) = object.get("workspaceFolders")
        && !workspace_folders.is_null()
    {
        let Some(items) = workspace_folders.as_array() else {
            return Err(Error::lsp(
                "initialize workspaceFolders must be an array or null",
            ));
        };
        if items.len() != 1
            || items[0].get("uri").and_then(Value::as_str) != Some(target.root_uri.as_str())
        {
            return Err(Error::lsp(format!(
                "daemon client workspaceFolders must contain only {}",
                target.root_uri
            )));
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
        serde_json::to_value(vec![WorkspaceFolder {
            uri: parse_lsp_uri(&target.root_uri, "workspace")?,
            name: target.workspace_name.clone(),
        }])
        .expect("workspace folders should serialize"),
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
            pairs.sort_by_key(|(left, _)| *left);
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
