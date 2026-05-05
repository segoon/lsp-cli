use super::LspClient;
use crate::error::{Error, Result, error_fn};
use crate::lsp::{InitializeResponse, parse_lsp_uri};
use lsp_types::notification::{DidOpenTextDocument, Initialized};
use lsp_types::request::{
    CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare,
    DocumentDiagnosticRequest, DocumentSymbolRequest, Formatting, GotoDeclaration,
    GotoDeclarationParams, GotoDefinition, Initialize, References, WorkspaceSymbolRequest,
};
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyItem, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams, ClientCapabilities, ClientInfo, DidOpenTextDocumentParams,
    DocumentDiagnosticParams, DocumentFormattingParams, DocumentSymbolParams, FormattingOptions,
    GeneralClientCapabilities, GotoDefinitionParams, InitializeParams, InitializedParams,
    PartialResultParams, Position, PositionEncodingKind, ReferenceContext, ReferenceParams,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, WindowClientCapabilities,
    WorkDoneProgressParams, WorkspaceClientCapabilities, WorkspaceFolder, WorkspaceSymbolParams,
};
use serde_json::{Value, json};
use std::path::Path;

impl LspClient {
    pub fn open_document(&mut self, path: &Path, uri: &str) -> Result<()> {
        if self.opened_documents.contains(uri) {
            return Ok(());
        }

        let text = std::fs::read_to_string(path).map_err(error_fn!(
            Error::unexpected,
            "failed to read {}",
            path.display()
        ))?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                parse_lsp_uri(uri, "document")?,
                language_id(path).to_string(),
                1,
                text,
            ),
        };
        self.send_notification::<DidOpenTextDocument>(&params)?;
        self.opened_documents.insert(uri.to_string());
        Ok(())
    }

    #[allow(deprecated)]
    pub fn initialize(
        &mut self,
        root_uri: &str,
        workspace_name: &str,
        want_server_status: bool,
    ) -> Result<InitializeResponse> {
        let root_uri = parse_lsp_uri(root_uri, "workspace")?;
        let workspace_folders = vec![WorkspaceFolder {
            uri: root_uri.clone(),
            name: workspace_name.to_string(),
        }];
        // Keep the advertised workspace-folder support aligned with the folder list we send.
        self.workspace_folders = Some(workspace_folders.clone());
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: None,
            root_uri: Some(root_uri.clone()),
            initialization_options: None,
            capabilities: ClientCapabilities {
                workspace: Some(WorkspaceClientCapabilities {
                    workspace_folders: Some(true),
                    ..Default::default()
                }),
                text_document: None,
                notebook_document: None,
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(want_server_status),
                    show_message: None,
                    show_document: None,
                }),
                general: Some(GeneralClientCapabilities {
                    position_encodings: Some(vec![PositionEncodingKind::UTF16]),
                    ..Default::default()
                }),
                experimental: Some(json!({ "serverStatusNotification": want_server_status })),
            },
            trace: None,
            workspace_folders: Some(workspace_folders),
            client_info: Some(ClientInfo {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            locale: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        let response = self.send_request::<Initialize>(&params)?;
        let response = InitializeResponse::from_raw_value(response).map_err(error_fn!(
            Error::lsp,
            "failed to decode initialize response"
        ))?;
        self.send_notification::<Initialized>(&InitializedParams {})?;
        self.drain_pending_server_requests()?;
        Ok(response)
    }

    pub fn workspace_symbol(&mut self, pattern: &str) -> Result<Value> {
        let params = WorkspaceSymbolParams {
            query: pattern.to_string(),
            ..Default::default()
        };
        self.send_request::<WorkspaceSymbolRequest>(&params)
    }

    pub fn document_symbol(&mut self, uri: &str) -> Result<Value> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier::new(parse_lsp_uri(uri, "document")?),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request::<DocumentSymbolRequest>(&params)
    }

    pub fn document_diagnostic(&mut self, uri: &str) -> Result<Value> {
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier::new(parse_lsp_uri(uri, "document")?),
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request::<DocumentDiagnosticRequest>(&params)
    }

    pub fn format_document(&mut self, uri: &str) -> Result<Value> {
        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier::new(parse_lsp_uri(uri, "document")?),
            options: FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                ..Default::default()
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.send_request::<Formatting>(&params)
    }

    pub fn references(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        include_declaration: bool,
    ) -> Result<Value> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams::new(
                TextDocumentIdentifier::new(parse_lsp_uri(uri, "document")?),
                Position::new(line, character),
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration,
            },
        };
        self.send_request::<References>(&params)
    }

    pub fn definition(&mut self, uri: &str, line: u32, character: u32) -> Result<Value> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier::new(parse_lsp_uri(uri, "document")?),
                Position::new(line, character),
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request::<GotoDefinition>(&params)
    }

    pub fn declaration(&mut self, uri: &str, line: u32, character: u32) -> Result<Value> {
        let params = GotoDeclarationParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier::new(parse_lsp_uri(uri, "document")?),
                Position::new(line, character),
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request::<GotoDeclaration>(&params)
    }

    pub fn prepare_call_hierarchy(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Value> {
        let params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier::new(parse_lsp_uri(uri, "document")?),
                Position::new(line, character),
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };
        self.send_request::<CallHierarchyPrepare>(&params)
    }

    pub fn incoming_calls(&mut self, item: &Value) -> Result<Value> {
        let item: CallHierarchyItem = serde_json::from_value(item.clone()).map_err(error_fn!(
            Error::lsp,
            "failed to decode call hierarchy item"
        ))?;
        let params = CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request::<CallHierarchyIncomingCalls>(&params)
    }

    pub fn outgoing_calls(&mut self, item: &Value) -> Result<Value> {
        let item: CallHierarchyItem = serde_json::from_value(item.clone()).map_err(error_fn!(
            Error::lsp,
            "failed to decode call hierarchy item"
        ))?;
        let params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        self.send_request::<CallHierarchyOutgoingCalls>(&params)
    }
}

fn language_id(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("c" | "h") => "c",
        Some("cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx") => "cpp",
        Some("cs") => "csharp",
        Some("go") => "go",
        Some("java") => "java",
        Some("js" | "mjs" | "cjs") => "javascript",
        Some("py") => "python",
        Some("rs") => "rust",
        Some("ts" | "mts" | "cts") => "typescript",
        _ => "plaintext",
    }
}
