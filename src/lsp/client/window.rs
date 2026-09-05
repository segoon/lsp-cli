use super::rpc::response_result;
use super::{IncomingMessage, LspClient, request_id, response_id};
use crate::error::{Error, Result};
use crate::lsp::{parse_lsp_uri, path_to_file_uri};
use lsp_types::notification::Cancel;
use lsp_types::request::{DocumentSymbolRequest, Request};
use lsp_types::{
    CancelParams, DocumentSymbolParams, NumberOrString, PartialResultParams,
    TextDocumentIdentifier, WorkDoneProgressParams,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

struct PendingDocument<'a> {
    index: usize,
    path: &'a Path,
    started: Instant,
}

impl PendingDocument<'_> {
    fn remaining(&self, timeout: Duration) -> Result<Duration> {
        match timeout.checked_sub(self.started.elapsed()) {
            Some(remaining) if !remaining.is_zero() => Ok(remaining),
            _ => Err(Error::lsp(format!(
                "timed out waiting for document symbols for {}",
                self.path.display()
            ))),
        }
    }
}

impl LspClient {
    /// Decode responses as they arrive, retaining only decoded results in file order.
    pub fn document_symbols_window<T>(
        &mut self,
        files: &[PathBuf],
        limit: NonZeroUsize,
        mut decode: impl FnMut(&Path, &Value) -> Result<T>,
    ) -> Result<Vec<T>> {
        let mut pending = BTreeMap::new();
        let result = self.collect_document_symbols(files, limit, &mut pending, &mut decode);
        if result.is_err() {
            // Cancellation is best effort: the server may already have replied. IDs
            // are never reused, so late replies cannot satisfy a subsequent request.
            for id in pending.keys() {
                // LSP cancellation permits string IDs, but these requests use numeric IDs.
                if let Ok(id) = i32::try_from(*id) {
                    let _ = self.write_notification::<Cancel>(&CancelParams {
                        id: NumberOrString::Number(id),
                    });
                }
            }
        }
        result
    }

    fn collect_document_symbols<'a, T>(
        &mut self,
        files: &'a [PathBuf],
        limit: NonZeroUsize,
        pending: &mut BTreeMap<u64, PendingDocument<'a>>,
        decode: &mut impl FnMut(&Path, &Value) -> Result<T>,
    ) -> Result<Vec<T>> {
        let mut next = files.iter().enumerate();
        let mut results = BTreeMap::new();
        loop {
            while pending.len() < limit.get() {
                Self::document_window_timeout(pending, self.timeout)?;
                let Some((index, path)) = next.next() else {
                    break;
                };
                let uri = path_to_file_uri(path)?;
                self.open_document_without_drain(path, &uri)
                    .map_err(|error| {
                        error.with_prefix(format!("failed to open {}", path.display()))
                    })?;
                Self::document_window_timeout(pending, self.timeout)?;
                let params = DocumentSymbolParams {
                    text_document: TextDocumentIdentifier::new(parse_lsp_uri(&uri, "document")?),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                };
                let started = Instant::now();
                let id = self.start_request::<DocumentSymbolRequest>(&params)?;
                pending.insert(
                    id,
                    PendingDocument {
                        index,
                        path,
                        started,
                    },
                );
            }
            if pending.is_empty() {
                return Ok(results.into_values().collect());
            }
            let timeout = Self::document_window_timeout(pending, self.timeout)?;
            match self.recv_message(timeout) {
                Ok(IncomingMessage::Message(message)) => {
                    Self::document_window_timeout(pending, self.timeout)?;
                    if let Some(id) = response_id(&message) {
                        if let Some(document) = pending.remove(&id) {
                            // Explicit server errors have historically skipped this file.
                            // Transport failures and timeouts must instead abort the scan.
                            if let Ok(response) =
                                response_result(DocumentSymbolRequest::METHOD, &message)
                            {
                                results.insert(
                                    document.index,
                                    decode(document.path, &response).map_err(|error| {
                                        error.with_prefix(format!(
                                            "failed to decode document symbols for {}",
                                            document.path.display()
                                        ))
                                    })?,
                                );
                            }
                        }
                    } else if let Some(id) = request_id(&message) {
                        self.handle_server_request(&id, &message)?;
                    } else {
                        self.handle_server_notification(&message)?;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Recheck absolute deadlines rather than resetting them on traffic.
                    Self::document_window_timeout(pending, self.timeout)?;
                }
                Ok(IncomingMessage::EndOfStream) => {
                    return Err(Error::lsp(
                        "LSP server closed while reading document symbols",
                    ));
                }
                Ok(IncomingMessage::Error(error)) => {
                    return Err(error.with_prefix("failed to read document symbols"));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(Error::lsp(
                        "LSP reader stopped while reading document symbols",
                    ));
                }
            }
        }
    }

    fn document_window_timeout(
        pending: &BTreeMap<u64, PendingDocument<'_>>,
        timeout: Duration,
    ) -> Result<Duration> {
        pending.values().try_fold(timeout, |remaining, document| {
            Ok(remaining.min(document.remaining(timeout)?))
        })
    }
}
