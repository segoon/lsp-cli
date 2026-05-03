use super::{
    ListSymbolsTarget, dedupe_symbol_matches, ensure_list_functions_support,
    ensure_list_symbols_support, list_symbols_target, preferred_function_name_matches,
    preferred_name_matches, render_paths_text, render_symbol_matches_text,
    render_symbol_matches_text_full, render_symbol_names_text, render_workspace_symbol_json_full,
    truncate_items,
};
use crate::lsp::SymbolMatch;
use crate::suggest::SuggestedLanguage;
use crate::test_support::TestDir;
use lsp_types::SymbolKind;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn matched(
    path: &str,
    line: u32,
    col: u32,
    name: &str,
    kind: SymbolKind,
    line_content: &str,
) -> SymbolMatch {
    SymbolMatch {
        name: name.to_string(),
        kind,
        path: PathBuf::from(path),
        line,
        col,
        line_content: line_content.to_string(),
        full_content: None,
    }
}

fn render_server() -> SuggestedLanguage {
    SuggestedLanguage {
        config_id: "pyright".to_string(),
        languages: vec!["python".to_string()],
        server: "pyright-langserver".to_string(),
        command: vec!["pyright-langserver".to_string(), "--stdio".to_string()],
        workspace_root: PathBuf::from("."),
        wait_for_index: false,
    }
}

#[test]
fn renders_grep_text_output() {
    assert_eq!(
        render_symbol_matches_text(&[matched(
            "src/main.rs",
            3,
            14,
            "main",
            SymbolKind::FUNCTION,
            "fn main() {",
        )]),
        "src/main.rs:3:14:fn main() {"
    );
}

#[test]
fn renders_empty_grep_text_output() {
    assert_eq!(render_symbol_matches_text(&[]), "");
}

#[test]
fn renders_symbol_names_text_output() {
    assert_eq!(
        render_symbol_names_text(&[
            matched(
                "src/main.rs",
                3,
                14,
                "main",
                SymbolKind::FUNCTION,
                "fn main() {"
            ),
            matched(
                "src/lib.rs",
                8,
                1,
                "helper",
                SymbolKind::METHOD,
                "fn helper() {}"
            ),
        ]),
        "main\nhelper"
    );
}

#[test]
fn renders_full_definition_text_output() {
    let mut matched = matched(
        "src/main.rs",
        3,
        14,
        "main",
        SymbolKind::FUNCTION,
        "fn main() {",
    );
    matched.full_content = Some("fn main() {\n    helper();\n}".to_string());

    assert_eq!(
        render_symbol_matches_text_full(&[matched]),
        "src/main.rs:3:14:\nfn main() {\n    helper();\n}"
    );
}

#[test]
fn renders_full_definition_json_output() {
    let mut matched = matched(
        "app/models.py",
        5,
        1,
        "Order",
        SymbolKind::CLASS,
        "class Order:",
    );
    matched.full_content =
        Some("class Order:\n    customer: str\n    items: list[OrderItem]".to_string());

    let rendered = render_workspace_symbol_json_full(
        "Order",
        std::path::Path::new("playground/python"),
        &BTreeSet::from(["python".to_string()]),
        &render_server(),
        &[matched],
    );

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
        json!({
            "query": "Order",
            "directory": "playground/python",
            "detected": ["python"],
            "server": {
                "name": "pyright-langserver",
                "languages": ["python"],
                "command": ["pyright-langserver", "--stdio"],
                "workspace_root": "."
            },
            "matches": [{
                "name": "Order",
                "kind": SymbolKind::CLASS,
                "path": "app/models.py",
                "line": 5,
                "col": 1,
                "line_content": "class Order:",
                "full_content": "class Order:\n    customer: str\n    items: list[OrderItem]"
            }]
        })
    );
}

#[test]
fn renders_paths_text_output() {
    assert_eq!(
        render_paths_text(&[PathBuf::from("src/lib.rs"), PathBuf::from("src/main.rs")]),
        "src/lib.rs\nsrc/main.rs"
    );
}

#[test]
fn truncates_items_to_limit() {
    let items = truncate_items(vec![1, 2, 3], 2, "lines");

    assert_eq!(items, vec![1, 2]);
}

#[test]
fn dedupes_symbol_matches_by_location_and_name() {
    let matched = matched(
        "src/main.rs",
        1,
        1,
        "main",
        SymbolKind::FUNCTION,
        "fn main() {}",
    );

    assert_eq!(
        dedupe_symbol_matches(vec![matched.clone(), matched.clone()]),
        vec![matched]
    );
}

#[test]
fn prefers_exact_name_matches_over_fuzzy_matches() {
    let exact = matched(
        "src/main.rs",
        1,
        1,
        "main",
        SymbolKind::FUNCTION,
        "fn main() {}",
    );
    let fuzzy = matched(
        "src/lsp/symbols.rs",
        1,
        1,
        "SymbolInformationItem",
        SymbolKind::STRUCT,
        "struct SymbolInformationItem {}",
    );

    assert_eq!(
        preferred_name_matches(vec![fuzzy, exact.clone()], "main"),
        vec![exact]
    );
}

#[test]
fn prefers_function_matches_for_function_queries() {
    let function = matched(
        "src/main.rs",
        1,
        1,
        "main",
        SymbolKind::FUNCTION,
        "fn main() {}",
    );
    let non_function = matched(
        "src/lib.rs",
        1,
        1,
        "main",
        SymbolKind::STRUCT,
        "struct main;",
    );

    assert_eq!(
        preferred_function_name_matches(vec![non_function, function.clone()], "main"),
        vec![function]
    );
}

#[test]
fn classifies_directory_for_list_symbols_query() {
    let dir = TestDir::new("list-symbols-dir");

    assert_eq!(
        list_symbols_target(dir.path()),
        Ok(ListSymbolsTarget::Directory)
    );
}

#[test]
fn classifies_file_for_list_symbols_query() {
    let dir = TestDir::new("list-symbols-file");
    let file = dir.write_file("src/main.rs", "fn main() {}\n");

    assert_eq!(list_symbols_target(&file), Ok(ListSymbolsTarget::File));
}

#[test]
fn rejects_missing_list_symbols_path() {
    let dir = TestDir::new("list-symbols-missing");
    let error =
        list_symbols_target(&dir.path().join("missing.rs")).expect_err("missing input should fail");

    assert!(error.contains("expected a file or directory path"));
    assert!(error.contains("does not exist"));
}

fn initialize_response(
    document_symbol_provider: Option<serde_json::Value>,
) -> crate::lsp::InitializeResponse {
    serde_json::from_value(json!({
        "capabilities": {
            "documentSymbolProvider": document_symbol_provider,
        }
    }))
    .expect("initialize response should decode")
}

#[test]
fn formats_list_functions_support_error_for_missing_document_symbol() {
    let error = ensure_list_functions_support(&initialize_response(None), "harper-ls")
        .expect_err("missing document symbol support should fail");

    assert_eq!(
        error,
        "harper-ls does not support list-functions because it does not advertise textDocument/documentSymbol"
    );
}

#[test]
fn formats_list_symbols_support_error_for_missing_document_symbol() {
    let error = ensure_list_symbols_support(&initialize_response(None), "harper-ls")
        .expect_err("missing document symbol support should fail");

    assert_eq!(
        error,
        "harper-ls does not support list-symbols because it does not advertise textDocument/documentSymbol"
    );
}
