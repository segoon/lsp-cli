use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolInformation, SymbolKind};
use serde::Deserialize;
use serde_json::Value;

use super::file_uri_to_path;

#[derive(Debug, Eq, PartialEq)]
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
            .filter(|symbol| is_function_symbol_kind(symbol.kind))
            .map(|symbol| symbol_information_to_match(symbol, source_cache))
            .collect(),
        DocumentSymbolResponse::Nested(symbols) => {
            let mut matches = Vec::new();
            for symbol in symbols {
                collect_document_symbol_matches(path, symbol, source_cache, &mut matches)?;
            }
            Ok(matches)
        }
    }
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

fn symbol_information_to_match(
    symbol: SymbolInformation,
    source_cache: &mut SourceCache,
) -> Result<SymbolMatch, String> {
    let path = file_uri_to_path(&symbol.location.uri.to_string())?;
    let line = symbol.location.range.start.line + 1;
    let col = symbol.location.range.start.character + 1;
    let line_index = usize::try_from(symbol.location.range.start.line)
        .map_err(|_| format!("line index overflow for {}", path.display()))?;
    let line_content = source_cache.line_content(&path, line_index);

    Ok(SymbolMatch {
        name: symbol.name,
        kind: symbol.kind,
        path,
        line,
        col,
        line_content,
    })
}

fn collect_document_symbol_matches(
    path: &Path,
    symbol: DocumentSymbol,
    source_cache: &mut SourceCache,
    matches: &mut Vec<SymbolMatch>,
) -> Result<(), String> {
    if is_function_symbol_kind(symbol.kind) {
        let line = symbol.selection_range.start.line + 1;
        let col = symbol.selection_range.start.character + 1;
        let line_index = usize::try_from(symbol.selection_range.start.line)
            .map_err(|_| format!("line index overflow for {}", path.display()))?;
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
            collect_document_symbol_matches(path, child, source_cache, matches)?;
        }
    }

    Ok(())
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
            Self::SymbolInformation(symbol) => Some(symbol.location.into_symbol_match(
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
            Self::Full(location) => Some(location.into_symbol_match(name, kind, source_cache)),
            Self::UriOnly { .. } => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Location {
    uri: String,
    range: Range,
}

impl Location {
    fn into_symbol_match(
        self,
        name: String,
        kind: SymbolKind,
        source_cache: &mut SourceCache,
    ) -> Result<SymbolMatch, String> {
        let path = file_uri_to_path(&self.uri)?;
        let line = self.range.start.line + 1;
        let col = self.range.start.character + 1;
        let line_index = usize::try_from(self.range.start.line)
            .map_err(|_| format!("line index overflow for {}", path.display()))?;
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
}

#[derive(Debug, Deserialize)]
struct Range {
    start: Position,
}

#[derive(Debug, Deserialize)]
struct Position {
    line: u32,
    character: u32,
}

#[cfg(test)]
mod tests {
    use super::{
        SourceCache, SymbolMatch, function_matches_from_document_response, is_function_symbol_kind,
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
}
