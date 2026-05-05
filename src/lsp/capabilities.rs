use lsp_types::ServerInfo;
use serde_json::Value;

use crate::error::{Error, Result};

#[derive(Debug)]
pub struct InitializeResponse {
    pub raw_result: Value,
    pub result: lsp_types::InitializeResult,
}

impl InitializeResponse {
    pub fn from_raw_value(raw_result: Value) -> std::result::Result<Self, serde_json::Error> {
        let result = serde_json::from_value(raw_result.clone())?;
        Ok(Self { raw_result, result })
    }

    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.result.server_info.as_ref()
    }

    pub fn capabilities_raw(&self) -> Option<&Value> {
        self.raw_result.get("capabilities")
    }

    pub fn capability(&self, path: &[&str]) -> Option<&Value> {
        let mut value = self.capabilities_raw()?;
        for part in path {
            value = value.get(*part)?;
        }
        Some(value)
    }
}

pub fn ensure_workspace_symbol_support(initialize: &InitializeResponse) -> Result<()> {
    if !supports(initialize.capability(&["workspaceSymbolProvider"])) {
        return Err(Error::lsp(
            "selected LSP server does not support workspace/symbol",
        ));
    }

    Ok(())
}

pub fn document_symbol_supported(initialize: &InitializeResponse) -> bool {
    supports(initialize.capability(&["documentSymbolProvider"]))
}

pub fn ensure_references_support(initialize: &InitializeResponse) -> Result<()> {
    if !supports(initialize.capability(&["referencesProvider"])) {
        return Err(Error::lsp(
            "selected LSP server does not support textDocument/references",
        ));
    }

    Ok(())
}

pub fn ensure_definition_support(initialize: &InitializeResponse) -> Result<()> {
    if !supports(initialize.capability(&["definitionProvider"])) {
        return Err(Error::lsp(
            "selected LSP server does not support textDocument/definition",
        ));
    }

    Ok(())
}

pub fn ensure_declaration_support(initialize: &InitializeResponse) -> Result<()> {
    if !supports(initialize.capability(&["declarationProvider"])) {
        return Err(Error::lsp(
            "selected LSP server does not support textDocument/declaration",
        ));
    }

    Ok(())
}

pub fn ensure_call_hierarchy_support(initialize: &InitializeResponse) -> Result<()> {
    if !supports(initialize.capability(&["callHierarchyProvider"])) {
        return Err(Error::lsp("selected LSP server does not support call hierarchy"));
    }

    Ok(())
}

pub fn diagnostics_supported(initialize: &InitializeResponse) -> bool {
    supports(initialize.capability(&["diagnosticProvider"]))
}

pub fn ensure_formatting_support(initialize: &InitializeResponse) -> Result<()> {
    if !supports(initialize.capability(&["documentFormattingProvider"])) {
        return Err(Error::lsp(
            "selected LSP server does not support textDocument/formatting",
        ));
    }

    Ok(())
}

fn supports(value: Option<&Value>) -> bool {
    !matches!(value, Some(Value::Bool(false) | Value::Null) | None)
}
