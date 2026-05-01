use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, DocumentSymbol,
    DocumentSymbolResponse, Location, LocationLink, SymbolInformation, SymbolKind,
};
use serde::Deserialize;
use serde_json::Value;

use super::file_uri_to_path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolMatch {
    pub name: String,
    pub kind: SymbolKind,
    pub path: PathBuf,
    pub line: u32,
    pub col: u32,
    pub line_content: String,
}

pub fn is_function_symbol_kind(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::METHOD | SymbolKind::CONSTRUCTOR | SymbolKind::FUNCTION | SymbolKind::OPERATOR
    )
}

pub fn should_skip_document_symbol_error(error: &str) -> bool {
    error.contains("file not found")
}

pub fn function_matches_from_document_response(
    response: &Value,
    path: &Path,
    source_cache: &mut SourceCache,
) -> Result<Vec<SymbolMatch>, String> {
    document_symbol_matches_from_response_with(
        response,
        path,
        source_cache,
        is_function_symbol_kind,
    )
}

pub fn document_symbol_matches_from_response(
    response: &Value,
    path: &Path,
    source_cache: &mut SourceCache,
) -> Result<Vec<SymbolMatch>, String> {
    document_symbol_matches_from_response_with(response, path, source_cache, |_| true)
}

pub fn symbol_matches_from_response(response: &Value) -> Result<Vec<SymbolMatch>, String> {
    if response.is_null() {
        return Ok(Vec::new());
    }

    let symbols: Vec<WorkspaceSymbolItem> = serde_json::from_value(response.clone())
        .map_err(|error| format!("failed to decode workspace/symbol response: {error}"))?;
    let mut source_cache = SourceCache::default();

    symbols
        .into_iter()
        .filter_map(|symbol| symbol.into_symbol_match(&mut source_cache))
        .collect()
}

pub fn location_matches_from_response(
    response: &Value,
    fallback_name: &str,
    fallback_kind: SymbolKind,
    source_cache: &mut SourceCache,
) -> Result<Vec<SymbolMatch>, String> {
    if response.is_null() {
        return Ok(Vec::new());
    }

    let response: LocationResponse = serde_json::from_value(response.clone())
        .map_err(|error| format!("failed to decode location response: {error}"))?;

    match response {
        LocationResponse::Scalar(location) => Ok(vec![location_to_symbol_match(
            &location,
            fallback_name.to_string(),
            fallback_kind,
            source_cache,
        )?]),
        LocationResponse::Array(locations) => locations
            .into_iter()
            .map(|location| {
                location_to_symbol_match(
                    &location,
                    fallback_name.to_string(),
                    fallback_kind,
                    source_cache,
                )
            })
            .collect(),
        LocationResponse::Link(links) => links
            .into_iter()
            .map(|link| {
                location_link_to_symbol_match(
                    &link,
                    fallback_name.to_string(),
                    fallback_kind,
                    source_cache,
                )
            })
            .collect(),
    }
}

pub fn prepare_call_hierarchy_response(response: &Value) -> Result<Vec<Value>, String> {
    if response.is_null() {
        return Ok(Vec::new());
    }

    serde_json::from_value(response.clone()).map_err(|error| {
        format!("failed to decode textDocument/prepareCallHierarchy response: {error}")
    })
}

pub fn call_hierarchy_matches_from_incoming_response(
    response: &Value,
    source_cache: &mut SourceCache,
) -> Result<Vec<SymbolMatch>, String> {
    if response.is_null() {
        return Ok(Vec::new());
    }

    let calls: Vec<CallHierarchyIncomingCall> =
        serde_json::from_value(response.clone()).map_err(|error| {
            format!("failed to decode callHierarchy/incomingCalls response: {error}")
        })?;

    calls
        .into_iter()
        .map(|call| call_hierarchy_item_to_match(call.from, source_cache))
        .collect()
}

pub fn call_hierarchy_matches_from_outgoing_response(
    response: &Value,
    source_cache: &mut SourceCache,
) -> Result<Vec<SymbolMatch>, String> {
    if response.is_null() {
        return Ok(Vec::new());
    }

    let calls: Vec<CallHierarchyOutgoingCall> =
        serde_json::from_value(response.clone()).map_err(|error| {
            format!("failed to decode callHierarchy/outgoingCalls response: {error}")
        })?;

    calls
        .into_iter()
        .map(|call| call_hierarchy_item_to_match(call.to, source_cache))
        .collect()
}

fn document_symbol_matches_from_response_with<F>(
    response: &Value,
    path: &Path,
    source_cache: &mut SourceCache,
    include: F,
) -> Result<Vec<SymbolMatch>, String>
where
    F: Copy + Fn(SymbolKind) -> bool,
{
    if response.is_null() {
        return Ok(Vec::new());
    }

    let response: DocumentSymbolResponse =
        serde_json::from_value(response.clone()).map_err(|error| {
            format!("failed to decode textDocument/documentSymbol response: {error}")
        })?;

    match response {
        DocumentSymbolResponse::Flat(symbols) => symbols
            .into_iter()
            .filter(|symbol| include(symbol.kind))
            .map(|symbol| symbol_information_to_match(symbol, source_cache))
            .collect(),
        DocumentSymbolResponse::Nested(symbols) => {
            let mut matches = Vec::new();
            for symbol in symbols {
                collect_document_symbol_matches(path, symbol, source_cache, &mut matches, include)?;
            }
            Ok(matches)
        }
    }
}

fn symbol_information_to_match(
    symbol: SymbolInformation,
    source_cache: &mut SourceCache,
) -> Result<SymbolMatch, String> {
    location_to_symbol_match(&symbol.location, symbol.name, symbol.kind, source_cache)
}

fn collect_document_symbol_matches<F>(
    path: &Path,
    symbol: DocumentSymbol,
    source_cache: &mut SourceCache,
    matches: &mut Vec<SymbolMatch>,
    include: F,
) -> Result<(), String>
where
    F: Copy + Fn(SymbolKind) -> bool,
{
    if include(symbol.kind) {
        let (line, col, line_index) = line_col_and_index(
            symbol.selection_range.start.line,
            symbol.selection_range.start.character,
            path,
        )?;
        let line_content = source_cache.line_content(path, line_index);

        matches.push(SymbolMatch {
            name: symbol.name.clone(),
            kind: symbol.kind,
            path: path.to_path_buf(),
            line,
            col,
            line_content,
        });
    }

    if let Some(children) = symbol.children {
        for child in children {
            collect_document_symbol_matches(path, child, source_cache, matches, include)?;
        }
    }

    Ok(())
}

fn location_to_symbol_match(
    location: &Location,
    name: String,
    kind: SymbolKind,
    source_cache: &mut SourceCache,
) -> Result<SymbolMatch, String> {
    let path = file_uri_to_path(&location.uri.to_string())?;
    let (line, col, line_index) = line_col_and_index(
        location.range.start.line,
        location.range.start.character,
        &path,
    )?;
    let line_content = source_cache.line_content(&path, line_index);

    Ok(SymbolMatch {
        name,
        kind,
        path,
        line,
        col,
        line_content,
    })
}

fn location_link_to_symbol_match(
    location: &LocationLink,
    name: String,
    kind: SymbolKind,
    source_cache: &mut SourceCache,
) -> Result<SymbolMatch, String> {
    let path = file_uri_to_path(&location.target_uri.to_string())?;
    let (line, col, line_index) = line_col_and_index(
        location.target_selection_range.start.line,
        location.target_selection_range.start.character,
        &path,
    )?;
    let line_content = source_cache.line_content(&path, line_index);

    Ok(SymbolMatch {
        name,
        kind,
        path,
        line,
        col,
        line_content,
    })
}

fn call_hierarchy_item_to_match(
    item: CallHierarchyItem,
    source_cache: &mut SourceCache,
) -> Result<SymbolMatch, String> {
    let path = file_uri_to_path(&item.uri.to_string())?;
    let (line, col, line_index) = line_col_and_index(
        item.selection_range.start.line,
        item.selection_range.start.character,
        &path,
    )?;
    let line_content = source_cache.line_content(&path, line_index);

    Ok(SymbolMatch {
        name: item.name,
        kind: item.kind,
        path,
        line,
        col,
        line_content,
    })
}

fn line_col_and_index(line: u32, character: u32, path: &Path) -> Result<(u32, u32, usize), String> {
    let line_index =
        usize::try_from(line).map_err(|_| format!("line index overflow for {}", path.display()))?;
    Ok((line + 1, character + 1, line_index))
}

#[derive(Debug, Default)]
pub struct SourceCache {
    lines: HashMap<PathBuf, Vec<String>>,
}

impl SourceCache {
    pub fn line_content(&mut self, path: &Path, line_index: usize) -> String {
        let entry = self.lines.entry(path.to_path_buf()).or_insert_with(|| {
            fs::read_to_string(path)
                .map(|contents| contents.lines().map(ToString::to_string).collect())
                .unwrap_or_default()
        });

        entry
            .get(line_index)
            .cloned()
            .unwrap_or_else(|| "<line unavailable>".to_string())
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkspaceSymbolItem {
    SymbolInformation(SymbolInformationItem),
    WorkspaceSymbol(WorkspaceSymbol),
}

impl WorkspaceSymbolItem {
    fn into_symbol_match(
        self,
        source_cache: &mut SourceCache,
    ) -> Option<Result<SymbolMatch, String>> {
        match self {
            Self::SymbolInformation(symbol) => Some(location_to_symbol_match(
                &symbol.location,
                symbol.name,
                symbol.kind,
                source_cache,
            )),
            Self::WorkspaceSymbol(symbol) => {
                symbol
                    .location
                    .into_symbol_match(symbol.name, symbol.kind, source_cache)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct SymbolInformationItem {
    name: String,
    kind: SymbolKind,
    location: Location,
}

#[derive(Debug, Deserialize)]
struct WorkspaceSymbol {
    name: String,
    kind: SymbolKind,
    location: WorkspaceSymbolLocation,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkspaceSymbolLocation {
    Full(Location),
    UriOnly {
        #[serde(rename = "uri")]
        _uri: Value,
    },
}

impl WorkspaceSymbolLocation {
    fn into_symbol_match(
        self,
        name: String,
        kind: SymbolKind,
        source_cache: &mut SourceCache,
    ) -> Option<Result<SymbolMatch, String>> {
        match self {
            Self::Full(location) => Some(location_to_symbol_match(
                &location,
                name,
                kind,
                source_cache,
            )),
            Self::UriOnly { .. } => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LocationResponse {
    Scalar(Location),
    Array(Vec<Location>),
    Link(Vec<LocationLink>),
}

#[cfg(test)]
mod tests {
    use super::{
        SourceCache, SymbolMatch, call_hierarchy_matches_from_incoming_response,
        call_hierarchy_matches_from_outgoing_response, document_symbol_matches_from_response,
        function_matches_from_document_response, is_function_symbol_kind,
        location_matches_from_response, prepare_call_hierarchy_response,
        symbol_matches_from_response,
    };
    use lsp_types::SymbolKind;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use url::Url;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lsp-cli-symbols-test-{}-{}",
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).expect("temp dir should be created");

            Self { path }
        }

        fn write_file(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dirs should be created");
            }

            fs::write(&path, contents).expect("file should be written");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn returns_placeholder_for_missing_line() {
        let dir = TestDir::new();
        let file = dir.write_file("main.rs", "fn main() {}\n");
        let mut cache = SourceCache::default();

        assert_eq!(cache.line_content(&file, 99), "<line unavailable>");
    }

    #[test]
    fn parses_workspace_symbol_locations() {
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
}
