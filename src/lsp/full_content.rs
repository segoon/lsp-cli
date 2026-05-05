use super::symbols::symbol_information_anchor;
use super::{SourceCache, SymbolMatch};
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
            .filter(|symbol| symbol.name == target.name)
            .filter_map(|symbol| {
                let line_col = symbol_information_anchor(&symbol.location, &symbol.name, path).ok();
                let range = symbol.location.range;
                if line_col.is_some_and(|(line, col, _)| line == target.line && col == target.col)
                    || range_contains_match(&range, target)
                {
                    Some((
                        range_size(&range),
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
    let direct_match = symbol.name == target.name
        && ((symbol.selection_range.start.line + 1 == target.line
            && symbol.selection_range.start.character + 1 == target.col)
            || range_contains_match(&symbol.range, target));

    let direct = direct_match.then(|| {
        (
            range_size(&symbol.range),
            source_cache.range_content_with_leading_comments(path, &symbol.range),
        )
    });

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

fn range_contains_match(range: &lsp_types::Range, target: &SymbolMatch) -> bool {
    let line = target.line.saturating_sub(1);
    let col = target.col.saturating_sub(1);
    let start = (range.start.line, range.start.character);
    let end = (range.end.line, range.end.character);
    let point = (line, col);

    start <= point && point <= end
}

fn range_size(range: &lsp_types::Range) -> u64 {
    let line_span = u64::from(range.end.line.saturating_sub(range.start.line));
    let col_span = u64::from(range.end.character.saturating_sub(range.start.character));
    line_span * 1_000_000 + col_span
}
