use crate::lsp::{
    LspClient, SourceCache, SymbolMatch, Utf16Position,
    call_hierarchy_matches_from_incoming_response, call_hierarchy_matches_from_outgoing_response,
    ensure_declaration_support, ensure_definition_support, ensure_implementation_support,
    ensure_references_support, ensure_type_definition_support,
};
use serde_json::Value;

use crate::error::Result;

#[derive(Clone, Copy)]
pub(in crate::commands) enum LocationQueryKind {
    References,
    Definition,
    Declaration,
    Implementation,
    TypeDefinition,
}

impl LocationQueryKind {
    pub(in crate::commands) fn label(self) -> &'static str {
        match self {
            Self::References => "references",
            Self::Definition => "definition",
            Self::Declaration => "declaration",
            Self::Implementation => "implementation",
            Self::TypeDefinition => "typeDefinition",
        }
    }

    pub(in crate::commands) fn ensure_support(
        self,
        initialize: &crate::lsp::InitializeResponse,
    ) -> Result<()> {
        match self {
            Self::References => ensure_references_support(initialize),
            Self::Definition => ensure_definition_support(initialize),
            Self::Declaration => ensure_declaration_support(initialize),
            Self::Implementation => ensure_implementation_support(initialize),
            Self::TypeDefinition => ensure_type_definition_support(initialize),
        }
    }

    pub(in crate::commands) fn query(
        self,
        client: &mut LspClient,
        uri: &str,
        position: Utf16Position,
    ) -> Result<Value> {
        match self {
            Self::References => client.references(uri, position, false),
            Self::Definition => client.definition(uri, position),
            Self::Declaration => client.declaration(uri, position),
            Self::Implementation => client.implementation(uri, position),
            Self::TypeDefinition => client.type_definition(uri, position),
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::commands) enum CallHierarchyDirection {
    Incoming,
    Outgoing,
}

impl CallHierarchyDirection {
    pub(in crate::commands) fn label(self) -> &'static str {
        match self {
            Self::Incoming => "callers",
            Self::Outgoing => "callees",
        }
    }

    pub(in crate::commands) fn query(self, client: &mut LspClient, item: &Value) -> Result<Value> {
        match self {
            Self::Incoming => client.incoming_calls(item),
            Self::Outgoing => client.outgoing_calls(item),
        }
    }

    pub(in crate::commands) fn decode(
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
