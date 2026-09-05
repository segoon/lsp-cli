use super::protocol::{
    error_response, fingerprint_value, id_key, local_server_request_response, message_method,
    normalize_initialize_params, request_id, respond_to_stop_request, response_id, stop_request_id,
    success_response, update_background_work_tracker, wants_background_work,
};
use super::{ClientPhase, Daemon, INVALID_REQUEST, SERVER_NOT_INITIALIZED};
use crate::error::{Error, Result};
use crate::lsp::transport::log_debug_message;
use crate::lsp::{STOP_METHOD, jsonrpc};
use lsp_types::request::{Initialize, Request};
use serde_json::Value;

impl Daemon {
    pub(super) fn handle_client_message(&mut self, message: &Value) -> Result<()> {
        log_debug_message(self.debug, "daemon client <- ", message);
        let method = message_method(message);
        let request_id = request_id(message);
        let response_id = response_id(message);

        if let Some(response_id) = response_id {
            let Some(client) = self.active_client.as_mut() else {
                return Ok(());
            };
            let key = id_key(&response_id);
            if client.pending_server_requests.remove(&key).is_some() {
                self.write_upstream_message(message)?;
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
                        self.write_upstream_message(message)?;
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

        self.write_upstream_message(message)
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
            self.restart_upstream()?;
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

        let Some(upstream) = self.upstream.as_mut() else {
            return Err(Error::unexpected("daemon failed to start LSP server"));
        };
        upstream.initialize_fingerprint = Some(fingerprint);

        let forwarded = jsonrpc(Some(request_id.clone()), Initialize::METHOD, &normalized)?;
        self.write_upstream_message(&forwarded)?;
        if let Some(client) = self.active_client.as_mut() {
            client.wants_background_work = wants_background_work;
            client.phase = ClientPhase::WaitingForInitialized {
                forward_to_upstream: true,
            };
            client.forwarded_client_requests.insert(id_key(&request_id));
        }
        Ok(())
    }

    fn handle_stop_request(&mut self, message: &Value) -> Result<()> {
        let Some(client) = self.active_client.as_mut() else {
            return Ok(());
        };

        respond_to_stop_request(&mut client.writer, message, self.debug)?;
        self.stop_requested = true;
        Ok(())
    }

    pub(super) fn handle_upstream_message(&mut self, message: &Value) -> Result<()> {
        log_debug_message(self.debug, "daemon upstream -> ", message);

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
                return self.write_client_response(message);
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
                return self.write_client_response(message);
            }

            let response = local_server_request_response(&request_id, method);
            return self.write_upstream_message(&response);
        }

        if self.active_client.is_some() {
            return self.write_client_response(message);
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
