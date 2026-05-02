use super::{
    dedupe_symbol_matches, ensure_list_functions_support, ensure_list_symbols_support,
    preferred_function_name_matches, preferred_name_matches, render_paths_text,
    render_symbol_matches_text, render_symbol_names_text, truncate_items,
    validate_list_symbols_file_path,
};
use crate::lsp::SymbolMatch;
use serde_json::json;
use crate::test_support::TestDir;
use lsp_types::SymbolKind;
use std::path::PathBuf;

#[test]
fn renders_grep_text_output() {
    assert_eq!(
        render_symbol_matches_text(&[SymbolMatch {
            name: "main".to_string(),
            kind: SymbolKind::FUNCTION,
            path: PathBuf::from("src/main.rs"),
            line: 3,
            col: 14,
            line_content: "fn main() {".to_string(),
        }]),
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
            SymbolMatch {
                name: "main".to_string(),
                kind: SymbolKind::FUNCTION,
                path: PathBuf::from("src/main.rs"),
                line: 3,
                col: 14,
                line_content: "fn main() {".to_string(),
            },
            SymbolMatch {
                name: "helper".to_string(),
                kind: SymbolKind::METHOD,
                path: PathBuf::from("src/lib.rs"),
                line: 8,
                col: 1,
                line_content: "fn helper() {}".to_string(),
            },
        ]),
        "main\nhelper"
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
    let matched = SymbolMatch {
        name: "main".to_string(),
        kind: SymbolKind::FUNCTION,
        path: PathBuf::from("src/main.rs"),
        line: 1,
        col: 1,
        line_content: "fn main() {}".to_string(),
    };

    assert_eq!(
        dedupe_symbol_matches(vec![matched.clone(), matched.clone()]),
        vec![matched]
    );
}

#[test]
fn prefers_exact_name_matches_over_fuzzy_matches() {
    let exact = SymbolMatch {
        name: "main".to_string(),
        kind: SymbolKind::FUNCTION,
        path: PathBuf::from("src/main.rs"),
        line: 1,
        col: 1,
        line_content: "fn main() {}".to_string(),
    };
    let fuzzy = SymbolMatch {
        name: "SymbolInformationItem".to_string(),
        kind: SymbolKind::STRUCT,
        path: PathBuf::from("src/lsp/symbols.rs"),
        line: 1,
        col: 1,
        line_content: "struct SymbolInformationItem {}".to_string(),
    };

    assert_eq!(
        preferred_name_matches(vec![fuzzy, exact.clone()], "main"),
        vec![exact]
    );
}

#[test]
fn prefers_function_matches_for_function_queries() {
    let function = SymbolMatch {
        name: "main".to_string(),
        kind: SymbolKind::FUNCTION,
        path: PathBuf::from("src/main.rs"),
        line: 1,
        col: 1,
        line_content: "fn main() {}".to_string(),
    };
    let non_function = SymbolMatch {
        name: "main".to_string(),
        kind: SymbolKind::STRUCT,
        path: PathBuf::from("src/lib.rs"),
        line: 1,
        col: 1,
        line_content: "struct main;".to_string(),
    };

    assert_eq!(
        preferred_function_name_matches(vec![non_function, function.clone()], "main"),
        vec![function]
    );
}

#[test]
fn rejects_directory_for_list_symbols_file_query() {
    let dir = TestDir::new("list-symbols");
    let error =
        validate_list_symbols_file_path(dir.path()).expect_err("directory input should fail");

    assert!(error.contains("expected a file path"));
    assert!(error.contains("is a directory"));
}

fn initialize_response(document_symbol_provider: Option<serde_json::Value>) -> crate::lsp::InitializeResponse {
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
