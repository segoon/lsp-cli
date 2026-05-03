use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct InitializeResponse {
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct ServerCapabilities {
    #[serde(rename = "workspaceSymbolProvider")]
    pub workspace_symbol_provider: Option<Value>,
    #[serde(rename = "documentSymbolProvider")]
    pub document_symbol_provider: Option<Value>,
    #[serde(rename = "referencesProvider")]
    pub references_provider: Option<Value>,
    #[serde(rename = "definitionProvider")]
    pub definition_provider: Option<Value>,
    #[serde(rename = "declarationProvider")]
    pub declaration_provider: Option<Value>,
    #[serde(rename = "callHierarchyProvider")]
    pub call_hierarchy_provider: Option<Value>,
    #[serde(rename = "diagnosticProvider")]
    pub diagnostic_provider: Option<Value>,
    #[serde(rename = "documentFormattingProvider")]
    pub document_formatting_provider: Option<Value>,
}

pub fn ensure_workspace_symbol_support(initialize: &InitializeResponse) -> Result<(), String> {
    if !supports(initialize.capabilities.workspace_symbol_provider.as_ref()) {
        return Err("selected LSP server does not support workspace/symbol".to_string());
    }

    Ok(())
}

pub fn document_symbol_supported(initialize: &InitializeResponse) -> bool {
    supports(initialize.capabilities.document_symbol_provider.as_ref())
}

pub fn ensure_references_support(initialize: &InitializeResponse) -> Result<(), String> {
    if !supports(initialize.capabilities.references_provider.as_ref()) {
        return Err("selected LSP server does not support textDocument/references".to_string());
    }

    Ok(())
}

pub fn ensure_definition_support(initialize: &InitializeResponse) -> Result<(), String> {
    if !supports(initialize.capabilities.definition_provider.as_ref()) {
        return Err("selected LSP server does not support textDocument/definition".to_string());
    }

    Ok(())
}

pub fn ensure_declaration_support(initialize: &InitializeResponse) -> Result<(), String> {
    if !supports(initialize.capabilities.declaration_provider.as_ref()) {
        return Err("selected LSP server does not support textDocument/declaration".to_string());
    }

    Ok(())
}

pub fn ensure_call_hierarchy_support(initialize: &InitializeResponse) -> Result<(), String> {
    if !supports(initialize.capabilities.call_hierarchy_provider.as_ref()) {
        return Err("selected LSP server does not support call hierarchy".to_string());
    }

    Ok(())
}

pub fn diagnostics_supported(initialize: &InitializeResponse) -> bool {
    supports(initialize.capabilities.diagnostic_provider.as_ref())
}

pub fn ensure_formatting_support(initialize: &InitializeResponse) -> Result<(), String> {
    if !supports(initialize.capabilities.document_formatting_provider.as_ref()) {
        return Err("selected LSP server does not support textDocument/formatting".to_string());
    }

    Ok(())
}

fn supports(value: Option<&Value>) -> bool {
    !matches!(value, Some(Value::Bool(false)) | None)
}
