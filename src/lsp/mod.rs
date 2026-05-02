mod custom_types;
mod jsonrpc;
mod capabilities;
mod client;
mod symbols;
pub(crate) mod transport;
mod uri;

pub use custom_types::{SERVER_STATUS_METHOD, STOP_METHOD, ServerStatusParams, StopParams};
pub use jsonrpc::jsonrpc;
pub use capabilities::{
    InitializeResponse, document_symbol_supported, ensure_call_hierarchy_support,
    ensure_declaration_support, ensure_definition_support, ensure_references_support,
    ensure_workspace_symbol_support,
};
pub use client::LspClient;
pub use symbols::{
    SourceCache, SymbolMatch, call_hierarchy_matches_from_incoming_response,
    call_hierarchy_matches_from_outgoing_response, document_symbol_matches_from_response,
    function_matches_from_document_response, is_function_symbol_kind,
    location_matches_from_response, prepare_call_hierarchy_response,
    should_skip_document_symbol_error, symbol_matches_from_response,
};
pub use uri::{file_uri_to_path, parse_lsp_uri, path_to_file_uri, workspace_name};
