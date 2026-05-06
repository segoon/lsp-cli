use super::SymbolMatch;
use crate::suggest::SuggestedLanguage;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

pub(crate) fn truncate_items<T>(mut items: Vec<T>, limit: usize, unit: &str) -> Vec<T> {
    if items.len() > limit {
        items.truncate(limit);
        eprintln!("output limit reached ({limit} {unit}); increase it with --limit");
    }

    items
}

pub(crate) fn render_symbol_matches_text(matches: &[SymbolMatch]) -> String {
    // Q: use error_fn
    matches
        .iter()
        .map(|matched| {
            format!(
                "{}:{}:{}:{}",
                matched.path.display(),
                matched.line,
                matched.col,
                matched.line_content
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn render_symbol_match_paths_text(matches: &[SymbolMatch]) -> String {
    let mut seen = HashSet::new();
    let paths = matches
        .iter()
        .filter(|matched| seen.insert(matched.path.clone()))
        .map(|matched| matched.path.clone())
        .collect::<Vec<_>>();

    render_paths_text(&paths)
}

pub(crate) fn render_symbol_matches_text_full(matches: &[SymbolMatch]) -> String {
    // Q: use error_fn
    matches
        .iter()
        .map(|matched| {
            format!(
                "{}:{}:{}:\n{}",
                matched.path.display(),
                matched.line,
                matched.col,
                matched
                    .full_content
                    .clone()
                    .unwrap_or_else(|| matched.line_content.clone())
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn render_symbol_names_text(matches: &[SymbolMatch]) -> String {
    matches
        .iter()
        .map(|matched| matched.name.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn render_paths_text(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn render_workspace_symbol_json(
    query: &str,
    directory: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> String {
    json!({
        "query": query,
        "directory": directory,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "matches": render_symbol_matches_json(matches, false),
    })
    .to_string()
}

pub(crate) fn render_workspace_symbol_json_full(
    query: &str,
    directory: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> String {
    json!({
        "query": query,
        "directory": directory,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "matches": render_symbol_matches_json(matches, true),
    })
    .to_string()
}

pub(crate) fn render_document_symbol_json(
    file: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> String {
    json!({
        "file": file,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "matches": render_symbol_matches_json(matches, false),
    })
    .to_string()
}

pub(crate) fn render_list_symbols_json(
    path: &Path,
    is_file: bool,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> String {
    if is_file {
        return render_document_symbol_json(path, detected_filetypes, server, matches);
    }

    json!({
        "directory": path,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "matches": render_symbol_matches_json(matches, false),
    })
    .to_string()
}

pub(crate) fn render_file_list_json(
    directory: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    files: &[PathBuf],
) -> String {
    json!({
        "directory": directory,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "files": files,
    })
    .to_string()
}

fn render_server_json(server: &SuggestedLanguage) -> Value {
    json!({
        "name": server.server,
        "languages": server.languages,
        "command": server.command,
        "workspace_root": server.workspace_root,
    })
}

fn render_symbol_matches_json(matches: &[SymbolMatch], include_full_content: bool) -> Vec<Value> {
    matches
        .iter()
        .map(|matched| {
            let mut value = json!({
                "name": matched.name,
                "kind": matched.kind,
                "path": matched.path,
                "line": matched.line,
                "col": matched.col,
                "line_content": matched.line_content,
            });
            if include_full_content {
                value["full_content"] = json!(
                    matched
                        .full_content
                        .clone()
                        .unwrap_or_else(|| matched.line_content.clone())
                );
            }
            value
        })
        .collect()
}
