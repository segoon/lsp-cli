mod capabilities;
mod client;
mod custom_types;
mod diagnostics;
mod full_content;
mod jsonrpc;
mod source_cache;
mod symbols;
pub(crate) mod transport;
mod uri;

pub use capabilities::{
    InitializeResponse, diagnostics_supported, document_symbol_supported,
    ensure_call_hierarchy_support, ensure_formatting_support,
    ensure_declaration_support, ensure_definition_support, ensure_references_support,
    ensure_workspace_symbol_support,
};
pub use client::LspClient;
pub use custom_types::{SERVER_STATUS_METHOD, STOP_METHOD, ServerStatusParams, StopParams};
pub use diagnostics::{
    DiagnosticMatch, diagnostic_matches_from_document_response,
    diagnostic_matches_from_notification,
};
pub use full_content::symbol_full_content_from_document_response;
pub use jsonrpc::jsonrpc;
pub use source_cache::SourceCache;
pub use symbols::{
    SymbolMatch, call_hierarchy_matches_from_incoming_response,
    call_hierarchy_matches_from_outgoing_response, document_symbol_matches_from_response,
    function_matches_from_document_response, is_function_symbol_kind,
    location_matches_from_response, location_matches_from_response_with_full_content,
    prepare_call_hierarchy_response, should_skip_document_symbol_error,
    symbol_matches_from_response,
};
pub use uri::{file_uri_to_path, parse_lsp_uri, path_to_file_uri, workspace_name};
