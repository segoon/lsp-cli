use crate::cli::{ListSymbolsArgs, LspWorkspaceQueryArgs, WorkspaceQueryArgs};
use crate::commands::common::{PreparedWorkspace, connect_lsp_client, prepare_workspace};
use crate::config::ConfigStore;
use crate::detect::matching_files;
use crate::lsp::{
    LspClient, SourceCache, SymbolMatch, document_symbol_matches_from_response,
    document_symbol_supported, ensure_call_hierarchy_support, ensure_document_symbol_support,
    ensure_workspace_symbol_support, function_matches_from_document_response,
    is_function_symbol_kind, location_matches_from_response, path_to_file_uri,
    prepare_call_hierarchy_response, should_skip_document_symbol_error,
    symbol_matches_from_response,
};
use crate::suggest::SuggestedLanguage;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod kinds;
mod render;

#[cfg(test)]
mod tests;

use kinds::{CallHierarchyDirection, LocationQueryKind, zero_based_col, zero_based_line};

pub(super) use render::{
    render_document_symbol_json, render_file_list_json, render_paths_text,
    render_symbol_matches_text, render_symbol_names_text, render_workspace_symbol_json,
    truncate_items,
};

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
    args: &LspWorkspaceQueryArgs,
    query: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.query.directory,
        args.query.lsp.as_deref(),
        args.query.lang.as_deref(),
        args.detach,
        args.query.wait_for_index,
        args.query.debug,
        args.query.timeout,
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
    args: &LspWorkspaceQueryArgs,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.query.directory,
        args.query.lsp.as_deref(),
        args.query.lang.as_deref(),
        args.detach,
        args.query.wait_for_index,
        args.query.debug,
        args.query.timeout,
        config,
        |workspace, initialize, client| {
            ensure_document_symbol_support(initialize)?;

            let files = matching_files(
                &args.query.directory,
                &config.filetypes,
                &workspace.allowed_filetypes,
            )
            .map_err(|error| {
                format!("failed to scan {}: {error}", args.query.directory.display())
            })?;
            let mut source_cache = SourceCache::default();
            let mut matches = Vec::new();

            for file in &files {
                let uri = path_to_file_uri(file)?;
                client.open_document(file, &uri).map_err(|error| {
                    format!(
                        "failed to open {} with {}: {error}",
                        file.display(),
                        workspace.server.server
                    )
                })?;
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
    validate_list_symbols_file_path(&args.file)?;

    let (workspace, matches) = with_initialized_client(
        &args.file,
        args.lsp.as_deref(),
        args.lang.as_deref(),
        args.detach,
        args.wait_for_index,
        args.debug,
        args.timeout,
        config,
        |workspace, initialize, client| {
            ensure_document_symbol_support(initialize)?;

            let uri = path_to_file_uri(&args.file)?;
            client.open_document(&args.file, &uri).map_err(|error| {
                format!(
                    "failed to open {} with {}: {error}",
                    args.file.display(),
                    workspace.server.server
                )
            })?;
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

fn validate_list_symbols_file_path(path: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "list-symbols expected a file path, but {} does not exist",
                path.display()
            )
        } else {
            format!("failed to inspect {}: {error}", path.display())
        }
    })?;

    if metadata.is_dir() {
        return Err(format!(
            "list-symbols expected a file path, but {} is a directory",
            path.display()
        ));
    }

    if !metadata.is_file() {
        return Err(format!(
            "list-symbols expected a regular file path, but {} is not a file",
            path.display()
        ));
    }

    Ok(())
}

pub(super) fn run_list_files_query(
    args: &WorkspaceQueryArgs,
    config: &ConfigStore,
) -> Result<FileListQueryResult, String> {
    let workspace = prepare_workspace(
        &args.directory,
        args.lsp.as_deref(),
        args.lang.as_deref(),
        config,
    )?;
    let files = matching_files(
        &args.directory,
        &config.filetypes,
        &workspace.allowed_filetypes,
    )
    .map_err(|error| format!("failed to scan {}: {error}", args.directory.display()))?;

    Ok(FileListQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        files,
    })
}

pub(super) fn run_references_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_named_location_query(args, name, LocationQueryKind::References, config)
}

pub(super) fn run_definition_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_named_location_query(args, name, LocationQueryKind::Definition, config)
}

pub(super) fn run_declaration_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_named_location_query(args, name, LocationQueryKind::Declaration, config)
}

pub(super) fn run_callers_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_call_hierarchy_query(args, name, CallHierarchyDirection::Incoming, config)
}

pub(super) fn run_callees_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    run_call_hierarchy_query(args, name, CallHierarchyDirection::Outgoing, config)
}

#[allow(clippy::too_many_arguments)]
fn with_initialized_client<T, F>(
    path: &Path,
    selected_server: Option<&str>,
    selected_language: Option<&str>,
    detach: bool,
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
    let workspace = prepare_workspace(path, selected_server, selected_language, config)?;
    let wait_for_index = wait_for_index_requested || workspace.server.wait_for_index;

    let mut client = connect_lsp_client(&workspace, detach, debug, timeout)?;
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
    args: &LspWorkspaceQueryArgs,
    name: &str,
    kind: LocationQueryKind,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.query.directory,
        args.query.lsp.as_deref(),
        args.query.lang.as_deref(),
        args.detach,
        args.query.wait_for_index,
        args.query.debug,
        args.query.timeout,
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
            let workspace_anchors = symbol_matches_from_response(&anchors)?;
            let anchors = select_named_anchors(
                workspace,
                initialize,
                client,
                config,
                NamedAnchorRequest {
                    directory: &args.query.directory,
                    name,
                    function_only: false,
                },
                workspace_anchors,
            )?;
            let mut source_cache = SourceCache::default();
            let mut matches = Vec::new();

            for anchor in anchors {
                let uri = path_to_file_uri(&anchor.path)?;
                client.open_document(&anchor.path, &uri).map_err(|error| {
                    format!(
                        "failed to open {} with {}: {error}",
                        anchor.path.display(),
                        workspace.server.server
                    )
                })?;
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
    args: &LspWorkspaceQueryArgs,
    name: &str,
    direction: CallHierarchyDirection,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let (workspace, matches) = with_initialized_client(
        &args.query.directory,
        args.query.lsp.as_deref(),
        args.query.lang.as_deref(),
        args.detach,
        args.query.wait_for_index,
        args.query.debug,
        args.query.timeout,
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
            let workspace_anchors = symbol_matches_from_response(&anchors)?;
            let anchors = select_named_anchors(
                workspace,
                initialize,
                client,
                config,
                NamedAnchorRequest {
                    directory: &args.query.directory,
                    name,
                    function_only: true,
                },
                workspace_anchors,
            )?;
            let mut source_cache = SourceCache::default();
            let mut matches = Vec::new();

            for anchor in anchors {
                let uri = path_to_file_uri(&anchor.path)?;
                client.open_document(&anchor.path, &uri).map_err(|error| {
                    format!(
                        "failed to open {} with {}: {error}",
                        anchor.path.display(),
                        workspace.server.server
                    )
                })?;
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

fn preferred_name_matches(matches: Vec<SymbolMatch>, name: &str) -> Vec<SymbolMatch> {
    let exact_matches = matches
        .iter()
        .filter(|matched| matched.name == name)
        .cloned()
        .collect::<Vec<_>>();

    if exact_matches.is_empty() {
        matches
    } else {
        exact_matches
    }
}

fn preferred_function_name_matches(matches: Vec<SymbolMatch>, name: &str) -> Vec<SymbolMatch> {
    let matches = preferred_name_matches(matches, name);
    matches
        .into_iter()
        .filter(|matched| is_function_symbol_kind(matched.kind))
        .collect()
}

fn select_named_anchors(
    workspace: &PreparedWorkspace,
    initialize: &crate::lsp::InitializeResponse,
    client: &mut LspClient,
    config: &ConfigStore,
    request: NamedAnchorRequest<'_>,
    workspace_anchors: Vec<SymbolMatch>,
) -> Result<Vec<SymbolMatch>, String> {
    if document_symbol_supported(initialize) {
        let document_anchors = exact_named_document_anchors(workspace, client, config, request)?;
        if !document_anchors.is_empty() {
            return Ok(document_anchors);
        }
    }

    Ok(if request.function_only {
        preferred_function_name_matches(workspace_anchors, request.name)
    } else {
        preferred_name_matches(workspace_anchors, request.name)
    })
}

#[derive(Clone, Copy)]
struct NamedAnchorRequest<'a> {
    directory: &'a Path,
    name: &'a str,
    function_only: bool,
}

fn exact_named_document_anchors(
    workspace: &PreparedWorkspace,
    client: &mut LspClient,
    config: &ConfigStore,
    request: NamedAnchorRequest<'_>,
) -> Result<Vec<SymbolMatch>, String> {
    let files = matching_files(
        request.directory,
        &config.filetypes,
        &workspace.allowed_filetypes,
    )
    .map_err(|error| format!("failed to scan {}: {error}", request.directory.display()))?;
    let mut source_cache = SourceCache::default();
    let mut matches = Vec::new();

    for file in &files {
        let uri = path_to_file_uri(file)?;
        client.open_document(file, &uri).map_err(|error| {
            format!(
                "failed to open {} with {}: {error}",
                file.display(),
                workspace.server.server
            )
        })?;
        let response = match client.document_symbol(&uri) {
            Ok(response) => response,
            Err(error) if should_skip_document_symbol_error(&error) => continue,
            Err(_) => continue,
        };
        let file_matches = if request.function_only {
            function_matches_from_document_response(&response, file, &mut source_cache)?
        } else {
            document_symbol_matches_from_response(&response, file, &mut source_cache)?
        };
        matches.extend(
            file_matches
                .into_iter()
                .filter(|matched| matched.name == request.name),
        );
    }

    Ok(dedupe_symbol_matches(matches))
}
