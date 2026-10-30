use super::LspClient;
use crate::error::{Error, Result};
use lsp_types::request::{SemanticTokensFullRequest, SemanticTokensRangeRequest};
use lsp_types::{
    PartialResultParams, SemanticTokensOptions, SemanticTokensParams, SemanticTokensRangeParams,
    TextDocumentIdentifier, WorkDoneProgressParams,
};
use serde_json::Value;

impl LspClient {
    pub(crate) fn semantic_tokens(
        &mut self,
        uri: &str,
        options: &SemanticTokensOptions,
        source: &str,
    ) -> Result<Value> {
        let text_document =
            TextDocumentIdentifier::new(crate::lsp::parse_lsp_uri(uri, "document")?);
        if crate::lsp::semantic_tokens::supports_full(options) {
            return self.send_request::<SemanticTokensFullRequest>(&SemanticTokensParams {
                text_document,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            });
        }
        let mut lines = source.split('\n');
        let last_line = lines.next_back().unwrap_or_default();
        let end_line = u32::try_from(lines.count())
            .map_err(|_| Error::invalid_input("file has too many lines to highlight"))?;
        let end_column = u32::try_from(last_line.encode_utf16().count())
            .map_err(|_| Error::invalid_input("file line is too long to highlight"))?;
        self.send_request::<SemanticTokensRangeRequest>(&SemanticTokensRangeParams {
            text_document,
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(end_line, end_column),
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
    }
}
