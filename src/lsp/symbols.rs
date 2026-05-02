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
    let name = symbol.name;
    let kind = symbol.kind;
    let path = file_uri_to_path(&symbol.location.uri.to_string())?;
    let (line, col, line_index) = symbol_information_anchor(&symbol.location, &name, &path)?;
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

fn symbol_information_anchor(
    location: &Location,
    name: &str,
    path: &Path,
) -> Result<(u32, u32, usize), String> {
    let lines = fs::read_to_string(path)
        .map(|contents| {
            contents
                .lines()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Flat SymbolInformation ranges often start at the whole declaration, not the symbol name.
    if let Some((line_index, character)) = name_offset_in_range(&lines, &location.range, name) {
        let line = u32::try_from(line_index)
            .map_err(|_| format!("line index overflow for {}", path.display()))?;
        return line_col_and_index(line, character, path);
    }

    line_col_and_index(
        location.range.start.line,
        location.range.start.character,
        path,
    )
}

fn name_offset_in_range(
    lines: &[String],
    range: &lsp_types::Range,
    name: &str,
) -> Option<(usize, u32)> {
    let start_line_index = usize::try_from(range.start.line).ok()?;
    let end_line_index = usize::try_from(range.end.line).ok()?;

    for line_index in start_line_index..=end_line_index {
        let line = lines.get(line_index)?;
        let start_col = if line_index == start_line_index {
            usize::try_from(range.start.character).ok()?
        } else {
            0
        };
        let end_col = if line_index == end_line_index {
            usize::try_from(range.end.character).ok()?
        } else {
            line.len()
        };
        let end_col = end_col.min(line.len());
        if start_col > end_col {
            continue;
        }

        let segment = &line[start_col..end_col];
        if let Some(offset) = identifier_name_offset(segment, name) {
            let character = u32::try_from(start_col + offset).ok()?;
            return Some((line_index, character));
        }
    }

    None
}

fn identifier_name_offset(line: &str, name: &str) -> Option<usize> {
    line.match_indices(name).find_map(|(offset, _)| {
        let before = line[..offset].chars().next_back();
        let after = line[offset + name.len()..].chars().next();
        if before.is_some_and(is_identifier_char) || after.is_some_and(is_identifier_char) {
            None
        } else {
            Some(offset)
        }
    })
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
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
mod tests;
