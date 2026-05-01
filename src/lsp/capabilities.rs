use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct InitializeResponse {
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Deserialize)]
pub struct ServerCapabilities {
    #[serde(rename = "workspaceSymbolProvider")]
    pub workspace_symbol_provider: Option<Value>,
    #[serde(rename = "documentSymbolProvider")]
    pub document_symbol_provider: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ServerStatusParams {
    pub health: String,
    pub quiescent: bool,
    pub message: Option<String>,
}

pub fn ensure_workspace_symbol_support(initialize: &InitializeResponse) -> Result<(), String> {
    if matches!(
        initialize.capabilities.workspace_symbol_provider,
        Some(Value::Bool(false)) | None
    ) {
        return Err("selected LSP server does not support workspace/symbol".to_string());
    }

    Ok(())
}

pub fn ensure_document_symbol_support(initialize: &InitializeResponse) -> Result<(), String> {
    if matches!(
        initialize.capabilities.document_symbol_provider,
        Some(Value::Bool(false)) | None
    ) {
        return Err("selected LSP server does not support textDocument/documentSymbol".to_string());
    }

    Ok(())
}
