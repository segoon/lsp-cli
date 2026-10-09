use serde_json::json;

use super::{
    QueryKind, observed_operations, validate_capability_contract, validate_variable_matches,
};

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

#[test]
fn capability_contract_ignores_operation_order() {
    use QueryKind::*;
    validate_capability_contract(
        &[ServerCapabilities, Definition],
        &[Definition, ServerCapabilities],
    )
    .expect("operation order should not affect support");
}

#[test]
fn capability_contract_reports_only_added_and_missing_operations() {
    use QueryKind::*;
    for (configured, observed, difference) in [
        (
            vec![ServerCapabilities],
            vec![ServerCapabilities, Implementation],
            "  Advertised by the server but missing from stored operations:\n    - implementation",
        ),
        (
            vec![ServerCapabilities, Definition],
            vec![ServerCapabilities],
            "  Stored as supported but not advertised by the server:\n    - definition",
        ),
        (
            vec![ServerCapabilities, Definition, References],
            vec![ServerCapabilities, TypeDefinition, Implementation],
            "  Advertised by the server but missing from stored operations:\n    - implementation\n    - type-definition\n  Stored as supported but not advertised by the server:\n    - references\n    - definition",
        ),
    ] {
        assert_eq!(
            validate_capability_contract(&configured, &observed),
            Err(format!(
                "server capabilities differ from stored supported-operations:\n{difference}"
            )),
        );
    }
}

#[test]
fn elp_contract_includes_advertised_implementation_support() {
    let manifest = crate::manifest::Manifest::load_validated(crate::repository_root())
        .expect("manifest should validate");
    let case = manifest
        .real_server_capabilities_cases()
        .find(|case| case.label() == "erlang/elp")
        .expect("ELP capability case should exist");
    // ELP 2026-10-05 advertises implementationProvider in its initialize response.
    let capabilities = json!({
        "workspaceSymbolProvider": true,
        "documentSymbolProvider": true,
        "referencesProvider": true,
        "callHierarchyProvider": true,
        "definitionProvider": true,
        "implementationProvider": true,
        "typeDefinitionProvider": true,
    });
    validate_capability_contract(
        case.supported_operations(),
        &observed_operations(&capabilities),
    )
    .expect("stored ELP operations should match the observed server response");
}
