use crate::cli::WorkspaceQueryArgs;
use crate::commands::common::prepare_workspace;
use crate::config::ConfigStore;
use crate::detect::matching_files;
use crate::lsp::{
    LspClient, SourceCache, SymbolMatch, ensure_document_symbol_support,
    ensure_workspace_symbol_support, function_matches_from_document_response, path_to_file_uri,
    should_skip_document_symbol_error, symbol_matches_from_response,
};
use crate::suggest::SuggestedLanguage;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) struct WorkspaceSymbolQueryResult {
    pub detected_filetypes: BTreeSet<String>,
    pub server: SuggestedLanguage,
    pub matches: Vec<SymbolMatch>,
}

pub(super) fn run_workspace_symbol_query(
    args: &WorkspaceQueryArgs,
    query: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let workspace = prepare_workspace(&args.directory, args.lsp.as_deref(), config)?;
    let wait_for_index = args.wait_for_index || workspace.server.wait_for_index;

    let mut client = LspClient::new(&workspace.server.command, args.debug, args.timeout)?;
    let initialize = client
        .initialize(
            &workspace.root_uri,
            &workspace.workspace_name,
            wait_for_index,
        )
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;
    ensure_workspace_symbol_support(&initialize)?;

    let response = (if wait_for_index {
        client.wait_for_background_work().map_err(|error| {
            format!(
                "failed to wait for background work with {}: {error}",
                workspace.server.server
            )
        })
    } else {
        Ok(())
    })
    .and_then(|()| {
        client
            .workspace_symbol(query)
            .map_err(|error| format!("failed to query {}: {error}", workspace.server.server))
    });
    let shutdown = client.shutdown();
    let response = response?;
    shutdown.map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches: symbol_matches_from_response(&response)?,
    })
}

pub(super) fn run_document_symbol_query(
    args: &WorkspaceQueryArgs,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let workspace = prepare_workspace(&args.directory, args.lsp.as_deref(), config)?;
    let files = matching_files(
        &args.directory,
        &config.filetypes,
        &workspace.detection.filetypes,
    )
    .map_err(|error| format!("failed to scan {}: {error}", args.directory.display()))?;
    let wait_for_index = args.wait_for_index || workspace.server.wait_for_index;

    let mut client = LspClient::new(&workspace.server.command, args.debug, args.timeout)?;
    let initialize = client
        .initialize(
            &workspace.root_uri,
            &workspace.workspace_name,
            wait_for_index,
        )
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;
    ensure_document_symbol_support(&initialize)?;

    let mut source_cache = SourceCache::default();
    let response = (if wait_for_index {
        client.wait_for_background_work().map_err(|error| {
            format!(
                "failed to wait for background work with {}: {error}",
                workspace.server.server
            )
        })
    } else {
        Ok(())
    })
    .and_then(|()| {
        let mut matches = Vec::new();
        for file in &files {
            let uri = path_to_file_uri(file)?;
            let response = match client.document_symbol(&uri) {
                Ok(response) => response,
                Err(error) if should_skip_document_symbol_error(&error) => continue,
                Err(error) => {
                    return Err(format!(
                        "failed to query {} for {}: {error}",
                        workspace.server.server,
                        file.display()
                    ));
                }
            };
            matches.extend(function_matches_from_document_response(
                &response,
                file,
                &mut source_cache,
            )?);
        }
        Ok(matches)
    });
    let shutdown = client.shutdown();
    let matches = response?;
    shutdown.map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}

pub(super) fn render_symbol_matches_text(matches: &[SymbolMatch]) -> String {
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

pub(super) fn render_symbol_names_text(matches: &[SymbolMatch]) -> String {
    matches
        .iter()
        .map(|matched| matched.name.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn render_workspace_symbol_json(
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
        "matches": render_symbol_matches_json(matches),
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

fn render_symbol_matches_json(matches: &[SymbolMatch]) -> Vec<Value> {
    matches
        .iter()
        .map(|matched| {
            json!({
                "name": matched.name,
                "kind": matched.kind,
                "path": matched.path,
                "line": matched.line,
                "col": matched.col,
                "line_content": matched.line_content,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{render_symbol_matches_text, render_symbol_names_text};
    use crate::lsp::SymbolMatch;
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
}
