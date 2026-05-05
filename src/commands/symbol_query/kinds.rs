use crate::lsp::{
    LspClient, SourceCache, SymbolMatch, call_hierarchy_matches_from_incoming_response,
    call_hierarchy_matches_from_outgoing_response, ensure_declaration_support,
    ensure_definition_support, ensure_references_support,
};
use serde_json::Value;

use crate::error::Result;

pub(super) fn zero_based_line(symbol: &SymbolMatch) -> u32 {
    symbol.line.saturating_sub(1)
}

pub(super) fn zero_based_col(symbol: &SymbolMatch) -> u32 {
    symbol.col.saturating_sub(1)
}

#[derive(Clone, Copy)]
pub(super) enum LocationQueryKind {
    References,
    Definition,
    Declaration,
}

impl LocationQueryKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::References => "references",
            Self::Definition => "definition",
            Self::Declaration => "declaration",
        }
    }

    pub(super) fn ensure_support(
        self,
        initialize: &crate::lsp::InitializeResponse,
    ) -> Result<()> {
        match self {
            Self::References => ensure_references_support(initialize),
            Self::Definition => ensure_definition_support(initialize),
            Self::Declaration => ensure_declaration_support(initialize),
        }
    }

    pub(super) fn query(
        self,
        client: &mut LspClient,
        uri: &str,
        anchor: &SymbolMatch,
    ) -> Result<Value> {
        match self {
            Self::References => {
                client.references(uri, zero_based_line(anchor), zero_based_col(anchor), false)
            }
            Self::Definition => client.definition(uri, zero_based_line(anchor), zero_based_col(anchor)),
            Self::Declaration => {
                client.declaration(uri, zero_based_line(anchor), zero_based_col(anchor))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum CallHierarchyDirection {
    Incoming,
    Outgoing,
}

impl CallHierarchyDirection {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Incoming => "callers",
            Self::Outgoing => "callees",
        }
    }

    pub(super) fn query(self, client: &mut LspClient, item: &Value) -> Result<Value> {
        match self {
            Self::Incoming => client.incoming_calls(item),
            Self::Outgoing => client.outgoing_calls(item),
        }
    }

    pub(super) fn decode(
        self,
        response: &Value,
        source_cache: &mut SourceCache,
    ) -> Result<Vec<SymbolMatch>> {
        match self {
            Self::Incoming => call_hierarchy_matches_from_incoming_response(response, source_cache),
            Self::Outgoing => call_hierarchy_matches_from_outgoing_response(response, source_cache),
        }
    }
}
