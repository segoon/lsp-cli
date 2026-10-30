use super::{SymbolMatch, WorkspaceSymbolQueryResult};
use crate::cli::WorkspaceQueryArgs;
use crate::error::{Error, Result};
use crate::lsp::SourceLine;
use crate::suggest::SuggestedLanguage;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

pub(crate) fn render_workspace_symbol_result(
    target: &crate::navigation::NavigationTarget,
    query: &WorkspaceQueryArgs,
    files_with_matches: bool,
    full: bool,
    result: WorkspaceSymbolQueryResult,
) -> Result<String> {
    let matches = truncate_items(
        result.matches,
        query.limit,
        if query.json { "items" } else { "lines" },
    );

    if query.json {
        let mut value = workspace_symbol_value(
            target.label(),
            &query.directory,
            &result.detected_filetypes,
            &result.server,
            &matches,
            full,
        )?;
        if let crate::navigation::NavigationTarget::Position {
            location,
            expected_name,
        } = target
        {
            value["at"] = json!(location.input());
            value["expected_name"] = json!(expected_name);
        }
        Ok(value.to_string())
    } else if files_with_matches {
        Ok(render_symbol_match_paths_text(&matches))
    } else if full {
        render_symbol_matches_text_full(&matches)
    } else {
        render_symbol_matches_text(&matches)
    }
}

pub(crate) fn truncate_items<T>(mut items: Vec<T>, limit: usize, unit: &str) -> Vec<T> {
    if items.len() > limit {
        items.truncate(limit);
        eprintln!("output limit reached ({limit} {unit}); increase it with --limit");
    }

    items
}

pub(crate) fn render_symbol_matches_text(matches: &[SymbolMatch]) -> Result<String> {
    Ok(matches
        .iter()
        .map(render_symbol_match_text)
        .collect::<Result<Vec<_>>>()?
        .join("\n"))
}

fn render_symbol_match_text(matched: &SymbolMatch) -> Result<String> {
    Ok(format!(
        "{}:{}:{}:{}",
        matched.path.display(),
        matched.position.line().one_based(),
        output_column(matched)?,
        matched.line_content
    ))
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

pub(crate) fn render_symbol_matches_text_full(matches: &[SymbolMatch]) -> Result<String> {
    Ok(matches
        .iter()
        .map(render_symbol_match_text_full)
        .collect::<Result<Vec<_>>>()?
        .join("\n\n"))
}

fn render_symbol_match_text_full(matched: &SymbolMatch) -> Result<String> {
    Ok(format!(
        "{}:{}:{}:\n{}",
        matched.path.display(),
        matched.position.line().one_based(),
        output_column(matched)?,
        matched
            .full_content
            .clone()
            .unwrap_or_else(|| matched.line_content.clone())
    ))
}

pub(crate) fn render_symbol_names_text(matches: &[SymbolMatch]) -> String {
    matches
        .iter()
        .filter_map(|matched| matched.name.clone())
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
) -> Result<String> {
    Ok(
        workspace_symbol_value(query, directory, detected_filetypes, server, matches, false)?
            .to_string(),
    )
}

#[cfg(test)]
pub(crate) fn render_workspace_symbol_json_full(
    query: &str,
    directory: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> Result<String> {
    Ok(
        workspace_symbol_value(query, directory, detected_filetypes, server, matches, true)?
            .to_string(),
    )
}

fn workspace_symbol_value(
    query: &str,
    directory: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
    full: bool,
) -> Result<Value> {
    Ok(json!({
        "query": query, "directory": directory, "detected": detected_filetypes,
        "server": render_server_json(server), "matches": render_symbol_matches_json(matches, full)?,
    }))
}

pub(crate) fn render_document_symbol_json(
    file: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> Result<String> {
    Ok(json!({
        "file": file,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "matches": render_symbol_matches_json(matches, false)?,
    })
    .to_string())
}

pub(crate) fn render_list_symbols_json(
    path: &Path,
    is_file: bool,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> Result<String> {
    if is_file {
        return render_document_symbol_json(path, detected_filetypes, server, matches);
    }

    Ok(json!({
        "directory": path,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "matches": render_symbol_matches_json(matches, false)?,
    })
    .to_string())
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

fn render_symbol_matches_json(
    matches: &[SymbolMatch],
    include_full_content: bool,
) -> Result<Vec<Value>> {
    matches
        .iter()
        .map(|matched| {
            let mut value = json!({
                "name": matched.name,
                "kind": matched.kind,
                "path": matched.path,
                "line": matched.position.line().one_based(),
                "col": output_column(matched)?,
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
            Ok(value)
        })
        .collect()
}

fn output_column(matched: &SymbolMatch) -> Result<u64> {
    // Use the captured source line, so output agrees with the text used during decoding.
    let column = if matched.line_content == "<line unavailable>" {
        return crate::lsp::SourceCache::default()
            .character_column(&matched.path, matched.position);
    } else {
        SourceLine::new(&matched.line_content).character_column(matched.position.column())
    };
    let Some(column) = column else {
        return Err(Error::invalid_input(format!(
            "cannot convert source position in {} at line {} to a character column: source text is unavailable or the server position is invalid",
            matched.path.display(),
            matched.position.line().one_based()
        )));
    };
    Ok(column)
}
