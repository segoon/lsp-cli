use super::protocol::{
    error_response, fingerprint_value, id_key, local_server_request_response, message_method,
    normalize_initialize_params, request_id, response_id, stop_request_id, success_response,
    update_background_work_tracker, wants_background_work,
};
use super::{
    ClientPhase, Daemon, INTERNAL_ERROR, INVALID_REQUEST, PendingInitialize, SERVER_NOT_INITIALIZED,
};
use crate::error::{Error, Result};
use crate::lsp::{STOP_METHOD, jsonrpc};
use lsp_types::request::{Initialize, Request};
use serde_json::Value;
use std::sync::Arc;

impl Daemon {
    pub(super) fn handle_client_message(&mut self, message: &Arc<Value>) -> Result<()> {
        self.logger.debug("daemon client <- ", Arc::clone(message));
        let method = message_method(message);
        let request_id = request_id(message);
        let response_id = response_id(message);

        if let Some(response_id) = response_id {
            let Some(client) = self.active_client.as_mut() else {
                return Ok(());
            };
            let key = id_key(&response_id);
            if client.pending_server_requests.remove(&key).is_some() {
                self.write_upstream_shared(message)?;
            }
            return Ok(());
        }

        match self.active_client.as_ref().map(|client| client.phase) {
            Some(ClientPhase::WaitingForInitialize) => {
                if stop_request_id(message).is_some() {
                    return self.handle_stop_request(message);
                }

                if method == Some("initialize") && request_id.is_some() {
                    return self.handle_initialize_request(message);
                }

                if method == Some("exit") {
                    self.disconnect_client()?;
                    return Ok(());
                }

                if let Some(request_id) = request_id {
                    return self.write_client_response(&error_response(
                        &request_id,
                        SERVER_NOT_INITIALIZED,
                        "daemon client must initialize before sending requests",
                    ));
                }

                return Ok(());
            }
            Some(ClientPhase::WaitingForInitialized {
                forward_to_upstream,
            }) => {
                if method == Some("initialized") {
                    if forward_to_upstream {
                        self.write_upstream_shared(message)?;
                    }
                    if let Some(client) = self.active_client.as_mut() {
                        client.phase = ClientPhase::Ready;
                    }
                    self.notify_client_if_background_ready(!forward_to_upstream)?;
                    return Ok(());
                }

                if let Some(request_id) = request_id {
                    return self.write_client_response(&error_response(
                        &request_id,
                        INVALID_REQUEST,
                        "daemon client must send initialized before other requests",
                    ));
                }

                return Ok(());
            }
            Some(ClientPhase::WaitingForUpstream) => {
                return self.handle_waiting_for_upstream(message, method, request_id);
            }
            Some(ClientPhase::WaitingForExit) => {
                if method == Some("exit") {
                    self.disconnect_client()?;
                }
                return Ok(());
            }
            Some(ClientPhase::Ready) | None => {}
        }

        if method == Some("shutdown") {
            let Some(request_id) = request_id else {
                return Ok(());
            };
            if let Some(client) = self.active_client.as_mut() {
                client.phase = ClientPhase::WaitingForExit;
            }
            return self.write_client_response(&success_response(&request_id, &Value::Null));
        }

        if method == Some("exit") {
            self.disconnect_client()?;
            return Ok(());
        }

        if method == Some(STOP_METHOD) {
            return self.handle_stop_request(message);
        }

        self.track_client_document_state(method, message.get("params"));

        if let Some(request_id) = request_id {
            let Some(client) = self.active_client.as_mut() else {
                return Ok(());
            };
            client.forwarded_client_requests.insert(id_key(&request_id));
        }

        self.write_upstream_shared(message)
    }

    fn handle_waiting_for_upstream(
        &mut self,
        message: &Value,
        method: Option<&str>,
        request_id: Option<Value>,
    ) -> Result<()> {
        if stop_request_id(message).is_some() {
            return self.handle_stop_request(message);
        }
        if method == Some("exit") {
            return self.disconnect_client();
        }
        if let Some(request_id) = request_id {
            return self.write_client_response(&error_response(
                &request_id,
                SERVER_NOT_INITIALIZED,
                "daemon is waiting for the LSP server to restart",
            ));
        }
        Ok(())
    }

    fn handle_initialize_request(&mut self, message: &Value) -> Result<()> {
        let Some(request_id) = request_id(message) else {
            return Ok(());
        };
        let Some(params) = message.get("params").cloned() else {
            return Err(Error::lsp("initialize request is missing params"));
        };
        let normalized = normalize_initialize_params(&params, &self.target)?;
        let fingerprint = fingerprint_value(&normalized);
        let wants_background_work = wants_background_work(&normalized);

        let should_restart = match self.upstream.as_ref() {
            Some(upstream) => {
                upstream.restart_required
                    || upstream
                        .initialize_fingerprint
                        .as_ref()
                        .is_some_and(|value| value != &fingerprint)
            }
            None => true,
        };

        if should_restart {
            self.pending_initialize = Some(PendingInitialize {
                request_id,
                normalized,
                fingerprint,
                wants_background_work,
            });
            if let Some(client) = self.active_client.as_mut() {
                client.phase = ClientPhase::WaitingForUpstream;
            }
            self.begin_restart()?;
            return Ok(());
        }

        if self
            .upstream
            .as_ref()
            .and_then(|upstream| upstream.initialize_fingerprint.as_ref())
            .is_some()
        {
            let Some(result) = self
                .upstream
                .as_ref()
                .and_then(|upstream| upstream.initialize_result.clone())
            else {
                return Err(Error::unexpected("daemon lost cached initialize result"));
            };
            self.write_client_response(&success_response(&request_id, &result))?;
            if let Some(client) = self.active_client.as_mut() {
                client.wants_background_work = wants_background_work;
                client.phase = ClientPhase::WaitingForInitialized {
                    forward_to_upstream: false,
                };
            }
            return Ok(());
        }

        self.forward_initialize(PendingInitialize {
            request_id,
            normalized,
            fingerprint,
            wants_background_work,
        })
    }

    pub(super) fn resume_pending_initialize(&mut self) -> Result<()> {
        let Some(initialize) = self.pending_initialize.take() else {
            return Ok(());
        };
        if self.active_client.is_none() {
            return Ok(());
        }
        self.forward_initialize(initialize)
    }

    fn forward_initialize(&mut self, initialize: PendingInitialize) -> Result<()> {
        let Some(upstream) = self.upstream.as_mut() else {
            return Err(Error::unexpected("daemon failed to start LSP server"));
        };
        upstream.initialize_fingerprint = Some(initialize.fingerprint);
        let forwarded = jsonrpc(
            Some(initialize.request_id.clone()),
            Initialize::METHOD,
            &initialize.normalized,
        )?;
        self.write_upstream_message(&forwarded)?;
        if let Some(client) = self.active_client.as_mut() {
            client.wants_background_work = initialize.wants_background_work;
            client.phase = ClientPhase::WaitingForInitialized {
                forward_to_upstream: true,
            };
            client
                .forwarded_client_requests
                .insert(id_key(&initialize.request_id));
        }
        Ok(())
    }

    pub(super) fn fail_pending_initialize(&mut self, message: &str) -> Result<()> {
        let Some(initialize) = self.pending_initialize.take() else {
            return Ok(());
        };
        let response = error_response(&initialize.request_id, INTERNAL_ERROR, message);
        let Some(write_id) = self.enqueue_client_response(&response)? else {
            return Ok(());
        };
        if let Some(client) = self.active_client.as_mut() {
            client.phase = ClientPhase::WaitingForExit;
            client.disconnect_after_write = Some(write_id);
        }
        Ok(())
    }

    fn handle_stop_request(&mut self, message: &Value) -> Result<()> {
        let Some(request_id) = stop_request_id(message) else {
            return Err(Error::lsp("daemon stop request is missing an id"));
        };
        let response = success_response(&request_id, &Value::Null);
        let Some(write_id) = self.enqueue_client_response(&response)? else {
            return Ok(());
        };
        if let Some(client) = self.active_client.as_mut() {
            client.stop_after_write = Some(write_id);
        }
        Ok(())
    }

    pub(super) fn handle_upstream_message(&mut self, message: &Arc<Value>) -> Result<()> {
        self.logger
            .debug("daemon upstream -> ", Arc::clone(message));

        if let Some(upstream) = self.upstream.as_mut() {
            update_background_work_tracker(message, &mut upstream.background_work)?;
        }

        if let Some(response_id) = response_id(message) {
            let response_key = id_key(&response_id);

            if self.orphaned_client_requests.remove(&response_key) {
                return Ok(());
            }

            let mut forwarded_client_request = false;
            let mut initialize_response = false;
            if let Some(client) = self.active_client.as_mut() {
                forwarded_client_request = client.forwarded_client_requests.remove(&response_key);
                initialize_response = forwarded_client_request
                    && matches!(
                        client.phase,
                        ClientPhase::WaitingForInitialized {
                            forward_to_upstream: true,
                        }
                    );
                if initialize_response && message.get("error").is_some() {
                    client.phase = ClientPhase::WaitingForExit;
                }
            }

            if initialize_response && let Some(upstream) = self.upstream.as_mut() {
                if message.get("error").is_some() {
                    upstream.initialize_fingerprint = None;
                    upstream.initialize_result = None;
                    upstream.restart_required = true;
                } else {
                    upstream.initialize_result = message.get("result").cloned();
                }
            }

            if forwarded_client_request {
                return self.enqueue_client_message(message).map(|_| ());
            }

            return Ok(());
        }

        if let Some(request_id) = request_id(message) {
            let Some(method) = message_method(message) else {
                return Err(Error::lsp("server request missing method"));
            };

            if matches!(
                method,
                "client/registerCapability" | "client/unregisterCapability"
            ) && let Some(upstream) = self.upstream.as_mut()
            {
                upstream.restart_required = true;
            }

            if let Some(client) = self.active_client.as_mut() {
                client
                    .pending_server_requests
                    .insert(id_key(&request_id), request_id.clone());
                return self.enqueue_client_message(message).map(|_| ());
            }

            let response = local_server_request_response(&request_id, method);
            return self.write_upstream_message(&response);
        }

        if self.active_client.is_some() {
            return self.enqueue_client_message(message).map(|_| ());
        }

        Ok(())
    }

    fn track_client_document_state(&mut self, method: Option<&str>, params: Option<&Value>) {
        let Some(client) = self.active_client.as_mut() else {
            return;
        };

        match method {
            Some("textDocument/didOpen") => {
                if let Some(uri) = params
                    .and_then(|value| value.get("textDocument"))
                    .and_then(|value| value.get("uri"))
                    .and_then(Value::as_str)
                {
                    client.open_documents.insert(uri.to_string());
                }
            }
            Some("textDocument/didClose") => {
                if let Some(uri) = params
                    .and_then(|value| value.get("textDocument"))
                    .and_then(|value| value.get("uri"))
                    .and_then(Value::as_str)
                {
                    client.open_documents.remove(uri);
                }
            }
            _ => {}
        }
    }
}
