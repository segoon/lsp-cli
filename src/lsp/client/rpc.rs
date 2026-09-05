use super::{IncomingMessage, LspClient, format_lsp_error, request_id, response_id};
use crate::error::{Error, Result};
use crate::lsp::{jsonrpc, transport::log_debug_message};
use crate::system_log::log_unexpected_error;
use lsp_types::request::{Initialize, Request};
use serde_json::Value;
use std::sync::mpsc::RecvTimeoutError;

impl LspClient {
    // Transmission does not drain the reader: a window must enforce deadlines even
    // when the server produces a continuous stream of notifications.
    pub(super) fn start_request<R: Request>(&mut self, params: &R::Params) -> Result<u64> {
        let id = self.next_request_id;
        self.next_request_id += 1;
        let message = jsonrpc(Some(id), R::METHOD, params)?;
        log_debug_message(self.debug, "-> ", &message);
        self.write_transport_message(&message)?;
        Ok(id)
    }

    pub(super) fn send_request<R>(&mut self, params: &R::Params) -> Result<Value>
    where
        R: Request,
    {
        if R::METHOD != Initialize::METHOD {
            self.drain_pending_server_requests()?;
        }

        let id = self.start_request::<R>(params)?;

        loop {
            match self.recv_message(self.timeout) {
                Ok(IncomingMessage::Message(message)) => {
                    if let Some(response_id) = response_id(&message) {
                        if response_id == id {
                            return response_result(R::METHOD, &message);
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
                        Error::lsp(format!("LSP server closed while waiting for {}", R::METHOD)),
                    ));
                }
                Ok(IncomingMessage::Error(error)) => {
                    let error =
                        error.with_prefix(format!("failed to read LSP message for {}", R::METHOD));
                    if error.should_log_as_unexpected() {
                        log_unexpected_error(&error.to_string());
                    }
                    return Err(error);
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(Error::lsp(format!("timed out waiting for {}", R::METHOD)));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(self.format_transport_wait_error(
                        R::METHOD,
                        Error::lsp(format!(
                            "LSP reader stopped while waiting for {}",
                            R::METHOD
                        )),
                    ));
                }
            }
        }
    }
}

pub(super) fn response_result(method: &str, message: &Value) -> Result<Value> {
    if let Some(error) = message.get("error") {
        return Err(Error::lsp(format_lsp_error(method, error)));
    }
    Ok(message.get("result").cloned().unwrap_or(Value::Null))
}
