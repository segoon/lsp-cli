use crate::cli::{ListSymbolsArgs, WorkspaceQueryArgs};
use crate::commands::common::{PreparedWorkspace, prepare_workspace};
use crate::config::ConfigStore;
use crate::detect::matching_files;
use crate::lsp::{
    LspClient, SourceCache, SymbolMatch, call_hierarchy_matches_from_incoming_response,
    call_hierarchy_matches_from_outgoing_response, document_symbol_matches_from_response,
    ensure_call_hierarchy_support, ensure_declaration_support, ensure_definition_support,
    ensure_document_symbol_support, ensure_references_support, ensure_workspace_symbol_support,
    function_matches_from_document_response, location_matches_from_response, path_to_file_uri,
    prepare_call_hierarchy_response, should_skip_document_symbol_error,
    symbol_matches_from_response,
};
use crate::suggest::SuggestedLanguage;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) struct WorkspaceSymbolQueryResult {
    pub detected_filetypes: BTreeSet<String>,
    pub server: SuggestedLanguage,
    pub matches: Vec<SymbolMatch>,
}

pub(super) struct FileListQueryResult {
    pub detected_filetypes: BTreeSet<String>,
    pub server: SuggestedLanguage,
    pub files: Vec<PathBuf>,
}

pub(super) fn run_workspace_symbol_query(
    args: &WorkspaceQueryArgs,
    query: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.directory,
        args.lsp.as_deref(),
        args.wait_for_index,
        args.debug,
        args.timeout,
        config,
        |workspace, initialize, client| {
            ensure_workspace_symbol_support(initialize)?;
            let response = client
                .workspace_symbol(query)
                .map_err(|error| format!("failed to query {}: {error}", workspace.server.server))?;
            symbol_matches_from_response(&response)
        },
    )?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}

pub(super) fn run_document_symbol_query(
    args: &WorkspaceQueryArgs,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.directory,
        args.lsp.as_deref(),
        args.wait_for_index,
        args.debug,
        args.timeout,
        config,
        |workspace, initialize, client| {
            ensure_document_symbol_support(initialize)?;

            let files = matching_files(
                &args.directory,
                &config.filetypes,
                &server_filetypes(&workspace.server),
            )
            .map_err(|error| format!("failed to scan {}: {error}", args.directory.display()))?;
            let mut source_cache = SourceCache::default();
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
        },
    )?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}

pub(super) fn run_file_symbol_query(
    args: &ListSymbolsArgs,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.file,
        args.lsp.as_deref(),
        args.wait_for_index,
        args.debug,
        args.timeout,
        config,
        |workspace, initialize, client| {
            ensure_document_symbol_support(initialize)?;

            let uri = path_to_file_uri(&args.file)?;
            let response = client.document_symbol(&uri).map_err(|error| {
                format!(
                    "failed to query {} for {}: {error}",
                    workspace.server.server,
                    args.file.display()
                )
            })?;
            let mut source_cache = SourceCache::default();
            document_symbol_matches_from_response(&response, &args.file, &mut source_cache)
        },
    )?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}

pub(super) fn run_list_files_query(
    args: &WorkspaceQueryArgs,
    config: &ConfigStore,
) -> Result<FileListQueryResult, String> {
    let workspace = prepare_workspace(&args.directory, args.lsp.as_deref(), config)?;
    let files = matching_files(
        &args.directory,
        &config.filetypes,
        &server_filetypes(&workspace.server),
    )
    .map_err(|error| format!("failed to scan {}: {error}", args.directory.display()))?;

    Ok(FileListQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        files,
    })
}

pub(super) fn run_references_query(
    args: &WorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_named_location_query(args, name, LocationQueryKind::References, config)
}

pub(super) fn run_definition_query(
    args: &WorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_named_location_query(args, name, LocationQueryKind::Definition, config)
}

pub(super) fn run_declaration_query(
    args: &WorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_named_location_query(args, name, LocationQueryKind::Declaration, config)
}

pub(super) fn run_callers_query(
    args: &WorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_call_hierarchy_query(args, name, CallHierarchyDirection::Incoming, config)
}

pub(super) fn run_callees_query(
    args: &WorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_call_hierarchy_query(args, name, CallHierarchyDirection::Outgoing, config)
}

pub(super) fn truncate_items<T>(mut items: Vec<T>, limit: usize, unit: &str) -> Vec<T> {
    if items.len() > limit {
        items.truncate(limit);
        eprintln!("output limit reached ({limit} {unit}); increase it with --limit");
    }

    items
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

pub(super) fn render_paths_text(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
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

pub(super) fn render_document_symbol_json(
    file: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> String {
    json!({
        "file": file,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "matches": render_symbol_matches_json(matches),
    })
    .to_string()
}

pub(super) fn render_file_list_json(
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

fn with_initialized_client<T, F>(
    path: &Path,
    selected_server: Option<&str>,
    wait_for_index_requested: bool,
    debug: bool,
    timeout: Duration,
    config: &ConfigStore,
    run: F,
) -> Result<(PreparedWorkspace, T), String>
where
    F: FnOnce(
        &PreparedWorkspace,
        &crate::lsp::InitializeResponse,
        &mut LspClient,
    ) -> Result<T, String>,
{
    let workspace = prepare_workspace(path, selected_server, config)?;
    let wait_for_index = wait_for_index_requested || workspace.server.wait_for_index;

    let mut client = LspClient::new(&workspace.server.command, debug, timeout)?;
    let initialize = client
        .initialize(
            &workspace.root_uri,
            &workspace.workspace_name,
            wait_for_index,
        )
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;

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
    .and_then(|()| run(&workspace, &initialize, &mut client));
    let shutdown = client.shutdown();
    let response = response?;
    shutdown.map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;

    Ok((workspace, response))
}

fn run_named_location_query(
    args: &WorkspaceQueryArgs,
    name: &str,
    kind: LocationQueryKind,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.directory,
        args.lsp.as_deref(),
        args.wait_for_index,
        args.debug,
        args.timeout,
        config,
        |workspace, initialize, client| {
            ensure_workspace_symbol_support(initialize)?;
            kind.ensure_support(initialize)?;

            let anchors = client.workspace_symbol(name).map_err(|error| {
                format!(
                    "failed to find matching symbols for {name:?} with {}: {error}",
                    workspace.server.server
                )
            })?;
            let anchors = symbol_matches_from_response(&anchors)?;
            let mut source_cache = SourceCache::default();
            let mut matches = Vec::new();

            for anchor in anchors {
                let uri = path_to_file_uri(&anchor.path)?;
                let response = kind.query(client, &uri, &anchor).map_err(|error| {
                    format!(
                        "failed to query {} for {} of {name:?}: {error}",
                        workspace.server.server,
                        kind.label()
                    )
                })?;
                matches.extend(location_matches_from_response(
                    &response,
                    &anchor.name,
                    anchor.kind,
                    &mut source_cache,
                )?);
            }

            Ok(dedupe_symbol_matches(matches))
        },
    )?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}

fn run_call_hierarchy_query(
    args: &WorkspaceQueryArgs,
    name: &str,
    direction: CallHierarchyDirection,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.directory,
        args.lsp.as_deref(),
        args.wait_for_index,
        args.debug,
        args.timeout,
        config,
        |workspace, initialize, client| {
            ensure_workspace_symbol_support(initialize)?;
            ensure_call_hierarchy_support(initialize)?;

            let anchors = client.workspace_symbol(name).map_err(|error| {
                format!(
                    "failed to find matching symbols for {name:?} with {}: {error}",
                    workspace.server.server
                )
            })?;
            let anchors = symbol_matches_from_response(&anchors)?;
            let mut source_cache = SourceCache::default();
            let mut matches = Vec::new();

            for anchor in anchors {
                let uri = path_to_file_uri(&anchor.path)?;
                let prepared = client
                    .prepare_call_hierarchy(&uri, zero_based_line(&anchor), zero_based_col(&anchor))
                    .map_err(|error| {
                        format!(
                            "failed to prepare call hierarchy with {} for {name:?}: {error}",
                            workspace.server.server
                        )
                    })?;
                let items = prepare_call_hierarchy_response(&prepared)?;

                for item in &items {
                    let response = direction.query(client, item).map_err(|error| {
                        format!(
                            "failed to query {} for {} of {name:?}: {error}",
                            workspace.server.server,
                            direction.label()
                        )
                    })?;
                    matches.extend(direction.decode(&response, &mut source_cache)?);
                }
            }

            Ok(dedupe_symbol_matches(matches))
        },
    )?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}

fn dedupe_symbol_matches(matches: Vec<SymbolMatch>) -> Vec<SymbolMatch> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for matched in matches {
        let key = (
            matched.path.clone(),
            matched.line,
            matched.col,
            matched.name.clone(),
        );
        if seen.insert(key) {
            deduped.push(matched);
        }
    }

    deduped
}

fn server_filetypes(server: &SuggestedLanguage) -> BTreeSet<String> {
    server.languages.iter().cloned().collect()
}

fn zero_based_line(symbol: &SymbolMatch) -> u32 {
    symbol.line.saturating_sub(1)
}

fn zero_based_col(symbol: &SymbolMatch) -> u32 {
    symbol.col.saturating_sub(1)
}

#[derive(Clone, Copy)]
enum LocationQueryKind {
    References,
    Definition,
    Declaration,
}

impl LocationQueryKind {
    fn label(self) -> &'static str {
        match self {
            Self::References => "references",
            Self::Definition => "definition",
            Self::Declaration => "declaration",
        }
    }

    fn ensure_support(self, initialize: &crate::lsp::InitializeResponse) -> Result<(), String> {
        match self {
            Self::References => ensure_references_support(initialize),
            Self::Definition => ensure_definition_support(initialize),
            Self::Declaration => ensure_declaration_support(initialize),
        }
    }

    fn query(
        self,
        client: &mut LspClient,
        uri: &str,
        anchor: &SymbolMatch,
    ) -> Result<Value, String> {
        match self {
            Self::References => {
                client.references(uri, zero_based_line(anchor), zero_based_col(anchor), false)
            }
            Self::Definition => {
                client.definition(uri, zero_based_line(anchor), zero_based_col(anchor))
            }
            Self::Declaration => {
                client.declaration(uri, zero_based_line(anchor), zero_based_col(anchor))
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CallHierarchyDirection {
    Incoming,
    Outgoing,
}

impl CallHierarchyDirection {
    fn label(self) -> &'static str {
        match self {
            Self::Incoming => "callers",
            Self::Outgoing => "callees",
        }
    }

    fn query(self, client: &mut LspClient, item: &Value) -> Result<Value, String> {
        match self {
            Self::Incoming => client.incoming_calls(item),
            Self::Outgoing => client.outgoing_calls(item),
        }
    }

    fn decode(
        self,
        response: &Value,
        source_cache: &mut SourceCache,
    ) -> Result<Vec<SymbolMatch>, String> {
        match self {
            Self::Incoming => call_hierarchy_matches_from_incoming_response(response, source_cache),
            Self::Outgoing => call_hierarchy_matches_from_outgoing_response(response, source_cache),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        dedupe_symbol_matches, render_paths_text, render_symbol_matches_text,
        render_symbol_names_text, truncate_items,
    };
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

    #[test]
    fn renders_paths_text_output() {
        assert_eq!(
            render_paths_text(&[PathBuf::from("src/lib.rs"), PathBuf::from("src/main.rs")]),
            "src/lib.rs\nsrc/main.rs"
        );
    }

    #[test]
    fn truncates_items_to_limit() {
        let items = truncate_items(vec![1, 2, 3], 2, "lines");

        assert_eq!(items, vec![1, 2]);
    }

    #[test]
    fn dedupes_symbol_matches_by_location_and_name() {
        let matched = SymbolMatch {
            name: "main".to_string(),
            kind: SymbolKind::FUNCTION,
            path: PathBuf::from("src/main.rs"),
            line: 1,
            col: 1,
            line_content: "fn main() {}".to_string(),
        };

        assert_eq!(
            dedupe_symbol_matches(vec![matched.clone(), matched.clone()]),
            vec![matched]
        );
    }
}
