use super::{
    SourceCache, SymbolMatch, call_hierarchy_matches_from_incoming_response,
    call_hierarchy_matches_from_outgoing_response, document_symbol_matches_from_response,
    function_matches_from_document_response, is_function_symbol_kind,
    location_matches_from_response, prepare_call_hierarchy_response, symbol_matches_from_response,
};
use crate::test_support::TestDir;
use lsp_types::SymbolKind;
use serde_json::json;
use url::Url;

#[test]
fn returns_placeholder_for_missing_line() {
    let dir = TestDir::new("symbols");
    let file = dir.write_file("main.rs", "fn main() {}\n");
    let mut cache = SourceCache::default();

    assert_eq!(cache.line_content(&file, 99), "<line unavailable>");
}

#[test]
fn parses_workspace_symbol_locations() {
    let dir = TestDir::new("symbols");
    let file = dir.write_file("src/lib.rs", "first line\nsecond line\n");
    let uri = Url::from_file_path(&file)
        .expect("file path should become URI")
        .to_string();

    let matches = symbol_matches_from_response(&json!([
        {
            "name": "symbol",
            "kind": 12,
            "location": {
                "uri": uri,
                "range": {
                    "start": { "line": 1, "character": 2 },
                    "end": { "line": 1, "character": 8 }
                }
            }
        }
    ]))
    .expect("response should parse");

    assert_eq!(
        matches,
        vec![SymbolMatch {
            name: "symbol".to_string(),
            kind: SymbolKind::FUNCTION,
            path: file,
            line: 2,
            col: 3,
            line_content: "second line".to_string(),
        }]
    );
}

#[test]
fn identifies_function_like_symbol_kinds() {
    assert!(is_function_symbol_kind(SymbolKind::METHOD));
    assert!(is_function_symbol_kind(SymbolKind::CONSTRUCTOR));
    assert!(is_function_symbol_kind(SymbolKind::FUNCTION));
    assert!(is_function_symbol_kind(SymbolKind::OPERATOR));
    assert!(!is_function_symbol_kind(SymbolKind::CLASS));
    assert!(!is_function_symbol_kind(SymbolKind::VARIABLE));
}

#[test]
fn parses_document_symbols_for_functions() {
    let dir = TestDir::new("symbols");
    let file = dir.write_file(
        "src/lib.rs",
        "struct S;\nfn first() {}\nimpl S {\n    fn second(&self) {}\n}\n",
    );
    let mut cache = SourceCache::default();

    let matches = function_matches_from_document_response(
        &json!([
            {
                "name": "S",
                "kind": 23,
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 3, "character": 1 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 7 },
                    "end": { "line": 0, "character": 8 }
                },
                "children": [
                    {
                        "name": "second",
                        "kind": 6,
                        "range": {
                            "start": { "line": 3, "character": 0 },
                            "end": { "line": 3, "character": 23 }
                        },
                        "selectionRange": {
                            "start": { "line": 3, "character": 7 },
                            "end": { "line": 3, "character": 13 }
                        }
                    }
                ]
            },
            {
                "name": "first",
                "kind": 12,
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 13 }
                },
                "selectionRange": {
                    "start": { "line": 1, "character": 3 },
                    "end": { "line": 1, "character": 8 }
                }
            }
        ]),
        &file,
        &mut cache,
    )
    .expect("document symbols should parse");

    assert_eq!(
        matches,
        vec![
            SymbolMatch {
                name: "second".to_string(),
                kind: SymbolKind::METHOD,
                path: file.clone(),
                line: 4,
                col: 8,
                line_content: "    fn second(&self) {}".to_string(),
            },
            SymbolMatch {
                name: "first".to_string(),
                kind: SymbolKind::FUNCTION,
                path: file,
                line: 2,
                col: 4,
                line_content: "fn first() {}".to_string(),
            },
        ]
    );
}

#[test]
fn parses_document_symbols_for_all_kinds() {
    let dir = TestDir::new("symbols");
    let file = dir.write_file("src/lib.rs", "struct S;\nfn first() {}\n");
    let mut cache = SourceCache::default();

    let matches = document_symbol_matches_from_response(
        &json!([
            {
                "name": "S",
                "kind": 23,
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 8 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 7 },
                    "end": { "line": 0, "character": 8 }
                }
            },
            {
                "name": "first",
                "kind": 12,
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 13 }
                },
                "selectionRange": {
                    "start": { "line": 1, "character": 3 },
                    "end": { "line": 1, "character": 8 }
                }
            }
        ]),
        &file,
        &mut cache,
    )
    .expect("document symbols should parse");

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].name, "S");
    assert_eq!(matches[1].name, "first");
}

#[test]
fn parses_location_links() {
    let dir = TestDir::new("symbols");
    let file = dir.write_file("src/lib.rs", "first line\nsecond line\n");
    let uri = Url::from_file_path(&file)
        .expect("file path should become URI")
        .to_string();
    let mut cache = SourceCache::default();

    let matches = location_matches_from_response(
        &json!([
            {
                "targetUri": uri,
                "targetRange": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 11 }
                },
                "targetSelectionRange": {
                    "start": { "line": 1, "character": 2 },
                    "end": { "line": 1, "character": 8 }
                }
            }
        ]),
        "symbol",
        SymbolKind::FUNCTION,
        &mut cache,
    )
    .expect("location links should parse");

    assert_eq!(matches[0].line, 2);
    assert_eq!(matches[0].col, 3);
}

#[test]
fn parses_prepare_call_hierarchy_response() {
    let items = prepare_call_hierarchy_response(&json!([
        {
            "name": "main",
            "kind": 12,
            "uri": "file:///tmp/main.rs",
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 10 }
            },
            "selectionRange": {
                "start": { "line": 0, "character": 3 },
                "end": { "line": 0, "character": 7 }
            }
        }
    ]))
    .expect("call hierarchy items should parse");

    assert_eq!(items.len(), 1);
}

#[test]
fn parses_call_hierarchy_incoming_calls() {
    let dir = TestDir::new("symbols");
    let file = dir.write_file("src/lib.rs", "fn caller() {}\n");
    let uri = Url::from_file_path(&file)
        .expect("file path should become URI")
        .to_string();
    let mut cache = SourceCache::default();

    let matches = call_hierarchy_matches_from_incoming_response(
        &json!([
            {
                "from": {
                    "name": "caller",
                    "kind": 12,
                    "uri": uri,
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 13 }
                    },
                    "selectionRange": {
                        "start": { "line": 0, "character": 3 },
                        "end": { "line": 0, "character": 9 }
                    }
                },
                "fromRanges": []
            }
        ]),
        &mut cache,
    )
    .expect("incoming calls should parse");

    assert_eq!(matches[0].name, "caller");
}

#[test]
fn parses_call_hierarchy_outgoing_calls() {
    let dir = TestDir::new("symbols");
    let file = dir.write_file("src/lib.rs", "fn callee() {}\n");
    let uri = Url::from_file_path(&file)
        .expect("file path should become URI")
        .to_string();
    let mut cache = SourceCache::default();

    let matches = call_hierarchy_matches_from_outgoing_response(
        &json!([
            {
                "to": {
                    "name": "callee",
                    "kind": 12,
                    "uri": uri,
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 13 }
                    },
                    "selectionRange": {
                        "start": { "line": 0, "character": 3 },
                        "end": { "line": 0, "character": 9 }
                    }
                },
                "fromRanges": []
            }
        ]),
        &mut cache,
    )
    .expect("outgoing calls should parse");

    assert_eq!(matches[0].name, "callee");
}
