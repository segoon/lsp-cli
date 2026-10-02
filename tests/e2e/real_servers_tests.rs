use serde_json::json;

use super::{QueryKind, observed_operations, validate_variable_matches};

#[test]
fn advertised_capabilities_map_to_operations() {
    let capabilities = json!({
        "documentFormattingProvider": false,
        "workspaceSymbolProvider": {},
        "documentSymbolProvider": true,
        "referencesProvider": null,
        "callHierarchyProvider": true,
        "definitionProvider": true,
    });

    assert_eq!(
        observed_operations(&capabilities),
        vec![
            QueryKind::ServerCapabilities,
            QueryKind::Grep,
            QueryKind::ListSymbols,
            QueryKind::ListFunctions,
            QueryKind::Callers,
            QueryKind::Callees,
            QueryKind::Definition,
        ]
    );
}

#[test]
fn variable_matches_accepts_any_array_cardinality_but_requires_matches() {
    for value in [json!({ "matches": [] }), json!({ "matches": [{}] })] {
        validate_variable_matches(QueryKind::Definition, "variable server", &value)
            .expect("matches arrays of either cardinality should be accepted");
    }
    validate_variable_matches(QueryKind::Definition, "variable server", &json!({}))
        .expect_err("a missing matches field should be rejected");
}
