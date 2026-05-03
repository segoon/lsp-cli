use super::{
    SymbolMatch, call_hierarchy_matches_from_incoming_response,
    call_hierarchy_matches_from_outgoing_response, document_symbol_matches_from_response,
    function_matches_from_document_response, is_function_symbol_kind,
    location_matches_from_response, location_matches_from_response_with_full_content,
    prepare_call_hierarchy_response, symbol_matches_from_response,
};
use crate::lsp::{SourceCache, symbol_full_content_from_document_response};
use crate::test_support::TestDir;
use lsp_types::SymbolKind;
use serde_json::json;
use url::Url;

struct SourceFixture {
    _dir: TestDir,
    file: std::path::PathBuf,
    uri: String,
    cache: SourceCache,
}

fn matched(
    path: &std::path::Path,
    line: u32,
    col: u32,
    name: &str,
    kind: SymbolKind,
    line_content: &str,
) -> SymbolMatch {
    SymbolMatch {
        name: name.to_string(),
        kind,
        path: path.to_path_buf(),
        line,
        col,
        line_content: line_content.to_string(),
        full_content: None,
    }
}

fn source_fixture(contents: &str) -> SourceFixture {
    let dir = TestDir::new("symbols");
    let file = dir.write_file("src/lib.rs", contents);
    let uri = Url::from_file_path(&file)
        .expect("file path should become URI")
        .to_string();

    SourceFixture {
        _dir: dir,
        file,
        uri,
        cache: SourceCache::default(),
    }
}

#[test]
fn returns_placeholder_for_missing_line() {
    let dir = TestDir::new("symbols");
    let file = dir.write_file("main.rs", "fn main() {}\n");
    let mut cache = SourceCache::default();

    assert_eq!(cache.line_content(&file, 99), "<line unavailable>");
}

#[test]
fn parses_workspace_symbol_locations() {
    let fixture = source_fixture("first line\nsecond line\n");

    let matches = symbol_matches_from_response(&json!([
        {
            "name": "symbol",
            "kind": 12,
            "location": {
                "uri": fixture.uri,
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
            path: fixture.file,
            line: 2,
            col: 3,
            line_content: "second line".to_string(),
            full_content: None,
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
    let mut fixture =
        source_fixture("struct S;\nfn first() {}\nimpl S {\n    fn second(&self) {}\n}\n");

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
        &fixture.file,
        &mut fixture.cache,
    )
    .expect("document symbols should parse");

    assert_eq!(
        matches,
        vec![
            SymbolMatch {
                name: "second".to_string(),
                kind: SymbolKind::METHOD,
                path: fixture.file.clone(),
                line: 4,
                col: 8,
                line_content: "    fn second(&self) {}".to_string(),
                full_content: None,
            },
            SymbolMatch {
                name: "first".to_string(),
                kind: SymbolKind::FUNCTION,
                path: fixture.file,
                line: 2,
                col: 4,
                line_content: "fn first() {}".to_string(),
                full_content: None,
            },
        ]
    );
}

#[test]
fn parses_document_symbols_for_all_kinds() {
    let mut fixture = source_fixture("struct S;\nfn first() {}\n");

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
        &fixture.file,
        &mut fixture.cache,
    )
    .expect("document symbols should parse");

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].name, "S");
    assert_eq!(matches[1].name, "first");
}

#[test]
fn parses_flat_document_symbols_at_symbol_name() {
    let mut fixture =
        source_fixture("pub fn format_order(order: &str) -> String { order.to_string() }\n");

    let matches = document_symbol_matches_from_response(
        &json!([
            {
                "name": "format_order",
                "kind": 12,
                "location": {
                    "uri": fixture.uri,
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 62 }
                    }
                }
            }
        ]),
        &fixture.file,
        &mut fixture.cache,
    )
    .expect("document symbols should parse");

    assert_eq!(matches[0].line, 1);
    assert_eq!(matches[0].col, 8);
    assert_eq!(
        matches[0].line_content,
        "pub fn format_order(order: &str) -> String { order.to_string() }"
    );
}

#[test]
fn parses_flat_document_symbols_inside_attributed_ranges() {
    let mut fixture = source_fixture("#[test]\nfn parses_callers_arguments() {}\n");

    let matches = function_matches_from_document_response(
        &json!([
            {
                "name": "parses_callers_arguments",
                "kind": 12,
                "location": {
                    "uri": fixture.uri,
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 1, "character": 32 }
                    }
                }
            }
        ]),
        &fixture.file,
        &mut fixture.cache,
    )
    .expect("document symbols should parse");

    assert_eq!(matches[0].line, 2);
    assert_eq!(matches[0].col, 4);
    assert_eq!(matches[0].line_content, "fn parses_callers_arguments() {}");
}

#[test]
fn parses_location_links() {
    let mut fixture = source_fixture("first line\nsecond line\n");

    let matches = location_matches_from_response(
        &json!([
            {
                "targetUri": fixture.uri,
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
        &mut fixture.cache,
    )
    .expect("location links should parse");

    assert_eq!(matches[0].line, 2);
    assert_eq!(matches[0].col, 3);
}

#[test]
fn keeps_full_content_from_definition_ranges() {
    let mut fixture = source_fixture("def build_sample_order():\n    return 1\n");

    let matches = location_matches_from_response_with_full_content(
        &json!([
            {
                "uri": fixture.uri,
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 1, "character": 12 }
                }
            }
        ]),
        "build_sample_order",
        SymbolKind::FUNCTION,
        &mut fixture.cache,
    )
    .expect("location response should parse");

    assert_eq!(
        matches[0].full_content.as_deref(),
        Some("def build_sample_order():\n    return 1")
    );
}

#[test]
fn extracts_function_body_from_nested_document_symbols() {
    let mut fixture = source_fixture("def build_sample_order() -> int:\n    return 1\n");
    let target = matched(
        &fixture.file,
        1,
        1,
        "build_sample_order",
        SymbolKind::FUNCTION,
        "def build_sample_order() -> int:",
    );

    let content = symbol_full_content_from_document_response(
        &json!([
            {
                "name": "build_sample_order",
                "kind": 12,
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 1, "character": 12 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 4 },
                    "end": { "line": 0, "character": 22 }
                }
            }
        ]),
        &fixture.file,
        &target,
        &mut fixture.cache,
    )
    .expect("document symbols should parse");

    assert_eq!(
        content.as_deref(),
        Some("def build_sample_order() -> int:\n    return 1")
    );
}

#[test]
fn extracts_class_body_with_attributes_from_nested_document_symbols() {
    let mut fixture = source_fixture(
        "class Order:\n    customer: str\n    items: list[str]\n\n    def total(self) -> int:\n        return len(self.items)\n",
    );
    let target = matched(
        &fixture.file,
        1,
        1,
        "Order",
        SymbolKind::CLASS,
        "class Order:",
    );

    let content = symbol_full_content_from_document_response(
        &json!([
            {
                "name": "Order",
                "kind": 5,
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 5, "character": 30 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 6 },
                    "end": { "line": 0, "character": 11 }
                },
                "children": [
                    {
                        "name": "customer",
                        "kind": 8,
                        "range": {
                            "start": { "line": 1, "character": 4 },
                            "end": { "line": 1, "character": 17 }
                        },
                        "selectionRange": {
                            "start": { "line": 1, "character": 4 },
                            "end": { "line": 1, "character": 12 }
                        }
                    }
                ]
            }
        ]),
        &fixture.file,
        &target,
        &mut fixture.cache,
    )
    .expect("document symbols should parse");

    assert_eq!(
        content.as_deref(),
        Some(
            "class Order:\n    customer: str\n    items: list[str]\n\n    def total(self) -> int:\n        return len(self.items)"
        )
    );
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
    let mut fixture = source_fixture("fn caller() {}\n");

    let matches = call_hierarchy_matches_from_incoming_response(
        &json!([
            {
                "from": {
                    "name": "caller",
                    "kind": 12,
                    "uri": fixture.uri,
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
        &mut fixture.cache,
    )
    .expect("incoming calls should parse");

    assert_eq!(matches[0].name, "caller");
}

#[test]
fn parses_call_hierarchy_outgoing_calls() {
    let mut fixture = source_fixture("fn callee() {}\n");

    let matches = call_hierarchy_matches_from_outgoing_response(
        &json!([
            {
                "to": {
                    "name": "callee",
                    "kind": 12,
                    "uri": fixture.uri,
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
        &mut fixture.cache,
    )
    .expect("outgoing calls should parse");

    assert_eq!(matches[0].name, "callee");
}
