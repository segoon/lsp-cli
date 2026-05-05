use super::{IncomingMessage, LspClient, request_id};
use crate::error::{Error, Result};
use crate::lsp::{SERVER_STATUS_METHOD, ServerStatusParams};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Instant;

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
    active_progress_tokens: BTreeSet<String>,
    finished_progress: bool,
}

impl LspClient {
    pub fn wait_for_background_work(&mut self) -> Result<()> {
        let started = Instant::now();
        let mut state = BuildIndexState::default();

        loop {
            let Some(remaining) = self.timeout.checked_sub(started.elapsed()) else {
                return Err(Error::lsp(timeout_error(&state)));
            };

            match self.recv_message(remaining) {
                Ok(IncomingMessage::Message(message)) => {
                    if let Some(outcome) = update_build_index_state(&message, &mut state)? {
                        return outcome;
                    }

                    if let Some(request_id) = request_id(&message) {
                        self.handle_server_request(&request_id, &message)?;
                    }
                }
                Ok(IncomingMessage::EndOfStream) => {
                    return Err(Error::lsp(
                        "LSP server closed while waiting for background work to finish",
                    ));
                }
                Ok(IncomingMessage::Error(error)) => {
                    return Err(error.with_prefix(
                        "failed to read LSP message while waiting for background work",
                    ));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(Error::lsp(timeout_error(&state)));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Error::lsp(
                        "LSP reader stopped while waiting for background work",
                    ));
                }
            }
        }
    }
}

fn update_build_index_state(
    message: &Value,
    state: &mut BuildIndexState,
) -> Result<Option<Result<()>>> {
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Ok(None);
    };

    match method {
        SERVER_STATUS_METHOD => {
            state.saw_server_status = true;
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let status: ServerStatusParams = serde_json::from_value(params)
                .map_err(|error| Error::lsp(format!("failed to decode {SERVER_STATUS_METHOD}: {error}")))?;

            if status.health == "error" {
                return Ok(Some(Err(Error::lsp(status.message.unwrap_or_else(|| {
                    "LSP server reported an indexing error".to_string()
                })))));
            }

            if status.quiescent {
                return Ok(Some(Ok(())));
            }
        }
        "window/workDoneProgress/create" => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let create: WorkDoneProgressCreateParams =
                serde_json::from_value(params).map_err(|error| {
                    Error::lsp(format!("failed to decode window/workDoneProgress/create: {error}"))
                })?;
            state.saw_work_done_progress = true;
            let _ = create.token;
        }
        "$/progress" => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let progress: ProgressParams = serde_json::from_value(params)
                .map_err(|error| Error::lsp(format!("failed to decode $/progress: {error}")))?;
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
