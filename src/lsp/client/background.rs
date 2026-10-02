use super::{IncomingMessage, LspClient, request_id};
use crate::error::{Error, Result, error_fn};
use crate::lsp::{SERVER_STATUS_METHOD, ServerStatusParams};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

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

#[derive(Clone, Copy)]
enum CompletionPolicy {
    Confirmed,
    BestEffort,
}

impl LspClient {
    pub fn wait_for_readiness_hint(
        &mut self,
        document_uri: &str,
        timeout: Duration,
    ) -> Result<bool> {
        if self.published_diagnostics.contains_key(document_uri) {
            return Ok(true);
        }

        let started = Instant::now();
        loop {
            let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                return Ok(false);
            };

            match self.recv_message(remaining) {
                Ok(IncomingMessage::Message(message)) => {
                    let is_target_diagnostic = diagnostic_uri(&message) == Some(document_uri);
                    if self.handle_server_notification(&message)? {
                        if is_target_diagnostic {
                            return Ok(true);
                        }
                        continue;
                    }
                    if notification_is_readiness_hint(&message)? {
                        return Ok(true);
                    }
                    if let Some(request_id) = request_id(&message) {
                        self.handle_server_request(&request_id, &message)?;
                    } else if message.get("method").is_none() {
                        // No request is outstanding here. Preserve an unexpected response so the
                        // operation that owns it can report or consume it instead of losing it.
                        self.pending_messages
                            .push_back(IncomingMessage::Message(message));
                        return Ok(false);
                    }
                }
                Ok(IncomingMessage::EndOfStream) | Err(RecvTimeoutError::Timeout) => {
                    return Ok(false);
                }
                Ok(IncomingMessage::Error(error)) => {
                    return Err(error.with_prefix("failed to wait for an LSP readiness signal"));
                }
                Err(RecvTimeoutError::Disconnected) => return Ok(false),
            }
        }
    }

    pub fn wait_for_background_work(&mut self) -> Result<()> {
        self.wait_for_background_work_with(CompletionPolicy::Confirmed)
    }

    pub fn wait_for_background_work_best_effort(&mut self) -> Result<()> {
        self.wait_for_background_work_with(CompletionPolicy::BestEffort)
    }

    fn wait_for_background_work_with(&mut self, policy: CompletionPolicy) -> Result<()> {
        let started = Instant::now();
        let mut state = BuildIndexState::default();

        loop {
            let Some(remaining) = self.timeout.checked_sub(started.elapsed()) else {
                return timeout_outcome(policy, &state);
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
                    return timeout_outcome(policy, &state);
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

fn diagnostic_uri(message: &Value) -> Option<&str> {
    (message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics"))
        .then(|| message.pointer("/params/uri").and_then(Value::as_str))
        .flatten()
}

fn notification_is_readiness_hint(message: &Value) -> Result<bool> {
    match message.get("method").and_then(Value::as_str) {
        Some(SERVER_STATUS_METHOD) => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let status: ServerStatusParams = serde_json::from_value(params).map_err(error_fn!(
                Error::lsp,
                "failed to decode {}",
                SERVER_STATUS_METHOD
            ))?;
            Ok(status.quiescent && status.health != "error")
        }
        Some("$/progress") => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let progress: ProgressParams = serde_json::from_value(params)
                .map_err(error_fn!(Error::lsp, "failed to decode $/progress"))?;
            Ok(progress.value.kind == "end")
        }
        _ => Ok(false),
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
            let status: ServerStatusParams = serde_json::from_value(params).map_err(error_fn!(
                Error::lsp,
                "failed to decode {}",
                SERVER_STATUS_METHOD
            ))?;

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
                serde_json::from_value(params).map_err(error_fn!(
                    Error::lsp,
                    "failed to decode window/workDoneProgress/create"
                ))?;
            state.saw_work_done_progress = true;
            let _ = create.token;
        }
        "$/progress" => {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let progress: ProgressParams = serde_json::from_value(params)
                .map_err(error_fn!(Error::lsp, "failed to decode $/progress"))?;
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

fn timeout_outcome(policy: CompletionPolicy, state: &BuildIndexState) -> Result<()> {
    match policy {
        CompletionPolicy::Confirmed => Err(Error::lsp(timeout_error(state))),
        // Best-effort servers have no terminal signal. Still consume the bounded wait window so
        // useful progress can finish, but do not misclassify the absence of a signal as failure.
        CompletionPolicy::BestEffort => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BuildIndexState, CompletionPolicy, diagnostic_uri, notification_is_readiness_hint,
        timeout_outcome,
    };
    use serde_json::json;

    #[test]
    fn recognizes_target_document_diagnostics() {
        let message = json!({
            "method": "textDocument/publishDiagnostics",
            "params": {"uri": "file:///workspace/main.rs", "diagnostics": []}
        });

        assert_eq!(diagnostic_uri(&message), Some("file:///workspace/main.rs"));
    }

    #[test]
    fn recognizes_only_completed_progress_as_a_readiness_hint() {
        let progress = |kind| {
            json!({
                "method": "$/progress",
                "params": {"token": "index", "value": {"kind": kind}}
            })
        };

        assert!(!notification_is_readiness_hint(&progress("begin")).expect("begin should decode"));
        assert!(
            !notification_is_readiness_hint(&progress("report")).expect("report should decode")
        );
        assert!(notification_is_readiness_hint(&progress("end")).expect("end should decode"));
    }

    #[test]
    fn recognizes_only_healthy_quiescent_server_status() {
        let status = |health, quiescent| {
            json!({
                "method": "experimental/serverStatus",
                "params": {"health": health, "quiescent": quiescent}
            })
        };

        assert!(!notification_is_readiness_hint(&status("ok", false)).expect("busy status"));
        assert!(notification_is_readiness_hint(&status("ok", true)).expect("ready status"));
        assert!(!notification_is_readiness_hint(&status("error", true)).expect("error status"));
    }

    #[test]
    fn best_effort_accepts_missing_completion_signal_after_bounded_wait() {
        timeout_outcome(CompletionPolicy::BestEffort, &BuildIndexState::default())
            .expect("best-effort completion accepts a missing signal");
        timeout_outcome(CompletionPolicy::Confirmed, &BuildIndexState::default())
            .expect_err("confirmed completion requires a signal");
    }
}
