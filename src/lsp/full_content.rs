use super::symbols::symbol_information_anchor;
use super::{SourceCache, SymbolMatch, Utf16Position, Utf16Range};
use crate::error::{Error, Result, error_fn};
use lsp_types::{DocumentSymbol, DocumentSymbolResponse};
use serde_json::Value;
use std::path::Path;

pub fn symbol_full_content_from_document_response(
    response: &Value,
    path: &Path,
    target: &SymbolMatch,
    source_cache: &mut SourceCache,
) -> Result<Option<String>> {
    if response.is_null() {
        return Ok(None);
    }

    let response: DocumentSymbolResponse =
        serde_json::from_value(response.clone()).map_err(error_fn!(
            Error::lsp,
            "failed to decode textDocument/documentSymbol response"
        ))?;

    match response {
        DocumentSymbolResponse::Flat(symbols) => Ok(symbols
            .into_iter()
            .filter(|symbol| {
                target
                    .name
                    .as_deref()
                    .is_none_or(|name| symbol.name == name)
            })
            .filter_map(|symbol| {
                let position = symbol_information_anchor(&symbol.location, &symbol.name, path);
                let range = Utf16Range::from_lsp(symbol.location.range);
                if position == target.position || range.contains_anchor(target.position) {
                    Some((
                        range.size_hint(),
                        source_cache.range_content_with_leading_comments(path, &range),
                    ))
                } else {
                    None
                }
            })
            .min_by_key(|(size, _)| *size)
            .map(|(_, content)| content)),
        DocumentSymbolResponse::Nested(symbols) => Ok(symbols
            .iter()
            .filter_map(|symbol| {
                matching_document_symbol_content(symbol, path, target, source_cache)
            })
            .min_by_key(|(size, _)| *size)
            .map(|(_, content)| content)),
    }
}

fn matching_document_symbol_content(
    symbol: &DocumentSymbol,
    path: &Path,
    target: &SymbolMatch,
    source_cache: &mut SourceCache,
) -> Option<(u64, String)> {
    let range = Utf16Range::from_lsp(symbol.range);
    let direct_match = target
        .name
        .as_deref()
        .is_none_or(|name| symbol.name == name)
        && (Utf16Position::from_lsp(symbol.selection_range.start) == target.position
            || range.contains_anchor(target.position));

    let direct = if direct_match {
        Some((
            range.size_hint(),
            source_cache.range_content_with_leading_comments(path, &range),
        ))
    } else {
        None
    };

    let nested = symbol
        .children
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|child| matching_document_symbol_content(child, path, target, source_cache))
        .min_by_key(|(size, _)| *size);

    match (direct, nested) {
        (Some(direct), Some(nested)) => Some(if direct.0 <= nested.0 { direct } else { nested }),
        (Some(direct), None) => Some(direct),
        (None, Some(nested)) => Some(nested),
        (None, None) => None,
    }
}
