use crate::cli::{ListSymbolsArgs, LspWorkspaceQueryArgs, WorkspaceQueryArgs};
use crate::commands::common::{PreparedWorkspace, connect_lsp_client, prepare_workspace};
use crate::config::ConfigStore;
use crate::detect::matching_files;
use crate::error::{Error, Result};
use crate::lsp::{
    LspClient, SourceCache, SymbolMatch, document_symbol_matches_from_response,
    document_symbol_supported, ensure_workspace_symbol_support,
    function_matches_from_document_response, is_function_symbol_kind, path_to_file_uri,
    should_skip_document_symbol_error, symbol_full_content_from_document_response,
    symbol_matches_from_response,
};
use crate::suggest::SuggestedLanguage;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod kinds;
mod named;
use named::{run_call_hierarchy_query, run_named_location_query};
mod render;

#[cfg(test)]
mod tests;

use kinds::{CallHierarchyDirection, LocationQueryKind};

pub(super) use render::{
    render_file_list_json, render_list_symbols_json, render_paths_text,
    render_symbol_match_paths_text, render_symbol_matches_text, render_symbol_names_text,
    render_workspace_symbol_json, render_workspace_symbol_result, truncate_items,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ListSymbolsTarget {
    File,
    Directory,
}

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
) -> Result<WorkspaceSymbolQueryResult> {
    let (workspace, matches) = with_initialized_client(
        &args.query.directory,
        args.query.selector.selected_server(),
        args.query.selector.selected_language(),
        args.detach,
        args.download,
        args.query.wait_for_index,
        args.query.debug,
        args.query.timeout,
        config,
        |workspace, initialize, client| {
            ensure_workspace_symbol_support(initialize)?;
            let response = client.workspace_symbol(query).map_err(|error| {
                error.with_prefix(format!("failed to query {}", workspace.server.server))
            })?;
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
) -> Result<WorkspaceSymbolQueryResult> {
    let (workspace, matches) = with_initialized_client(
        &args.query.directory,
        args.query.selector.selected_server(),
        args.query.selector.selected_language(),
        args.detach,
        args.download,
        args.query.wait_for_index,
        args.query.debug,
        args.query.timeout,
        config,
        |workspace, initialize, client| {
            ensure_document_symbol_support(initialize, &workspace.server.server, "list-functions")?;

            let files = scan_workspace_files(&args.query.directory, config, workspace)?;
            let mut source_cache = SourceCache::default();
            let mut matches = Vec::new();

            for file in &files {
                let uri = open_document_for(client, file, &workspace.server.server)?;
                let response = match client.document_symbol(&uri) {
                    Ok(response) => response,
                    Err(error) if should_skip_document_symbol_error(&error.to_string()) => continue,
                    Err(error) => {
                        return Err(Error::lsp(format!(
                            "failed to query {} for {}: {error}",
                            workspace.server.server,
                            file.display()
                        )));
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

pub(super) fn run_list_symbols_query(
    args: &ListSymbolsArgs,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult> {
    let target = list_symbols_target(&args.path)?;

    let (workspace, matches) = with_initialized_client(
        &args.path,
        args.server.server(),
        args.server.language(),
        args.detach,
        args.server.download,
        args.wait_for_index,
        args.server.debug,
        args.timeout,
        config,
        |workspace, initialize, client| {
            collect_list_symbol_matches(target, args, config, workspace, initialize, client)
        },
    )?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}

fn collect_list_symbol_matches(
    target: ListSymbolsTarget,
    args: &ListSymbolsArgs,
    config: &ConfigStore,
    workspace: &PreparedWorkspace,
    initialize: &crate::lsp::InitializeResponse,
    client: &mut LspClient,
) -> Result<Vec<SymbolMatch>> {
    ensure_document_symbol_support(initialize, &workspace.server.server, "list-symbols")?;

    let files = match target {
        ListSymbolsTarget::File => vec![args.path.clone()],
        ListSymbolsTarget::Directory => scan_workspace_files(&args.path, config, workspace)?,
    };

    let mut source_cache = SourceCache::default();
    let mut matches = Vec::new();

    for file in &files {
        let uri = open_document_for(client, file, &workspace.server.server)?;
        let response = match client.document_symbol(&uri) {
            Ok(response) => response,
            Err(error)
                if target == ListSymbolsTarget::Directory
                    && should_skip_document_symbol_error(&error.to_string()) =>
            {
                continue;
            }
            Err(error) => {
                return Err(Error::lsp(format!(
                    "failed to query {} for {}: {error}",
                    workspace.server.server,
                    file.display()
                )));
            }
        };
        matches.extend(document_symbol_matches_from_response(
            &response,
            file,
            &mut source_cache,
        )?);
    }

    Ok(matches)
}

fn scan_workspace_files(
    directory: &Path,
    config: &ConfigStore,
    workspace: &PreparedWorkspace,
) -> Result<Vec<PathBuf>> {
    matching_files(directory, &config.filetypes, &workspace.allowed_filetypes).map_err(|error| {
        Error::unexpected(format!("failed to scan {}: {error}", directory.display()))
    })
}

fn open_document_for(client: &mut LspClient, path: &Path, server_name: &str) -> Result<String> {
    let uri = path_to_file_uri(path)?;
    client.open_document(path, &uri).map_err(|error| {
        error.with_prefix(format!(
            "failed to open {} with {server_name}",
            path.display()
        ))
    })?;
    Ok(uri)
}

pub(super) fn list_symbols_target(path: &Path) -> Result<ListSymbolsTarget> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::invalid_input(format!(
                "list-symbols expected a file or directory path, but {} does not exist",
                path.display()
            ))
        } else {
            Error::unexpected(format!("failed to inspect {}: {error}", path.display()))
        }
    })?;

    if metadata.is_dir() {
        return Ok(ListSymbolsTarget::Directory);
    }

    if metadata.is_file() {
        return Ok(ListSymbolsTarget::File);
    }

    Err(Error::invalid_input(format!(
        "list-symbols expected a regular file or directory path, but {} is not supported",
        path.display()
    )))
}

fn ensure_document_symbol_support(
    initialize: &crate::lsp::InitializeResponse,
    server_name: &str,
    command: &str,
) -> Result<()> {
    if !document_symbol_supported(initialize) {
        return Err(Error::lsp(format!(
            "{server_name} does not support {command} because it does not advertise textDocument/documentSymbol"
        )));
    }

    Ok(())
}

pub(super) fn run_list_files_query(
    args: &WorkspaceQueryArgs,
    config: &ConfigStore,
) -> Result<FileListQueryResult> {
    let workspace = prepare_workspace(
        &args.directory,
        args.selector.selected_server(),
        args.selector.selected_language(),
        false,
        config,
    )?;
    let files = scan_workspace_files(&args.directory, config, &workspace)?;

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
) -> Result<WorkspaceSymbolQueryResult> {
    run_named_location_query(args, name, LocationQueryKind::References, false, config)
}

pub(super) fn run_definition_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    full: bool,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult> {
    run_named_location_query(args, name, LocationQueryKind::Definition, full, config)
}

pub(super) fn run_declaration_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    full: bool,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult> {
    run_named_location_query(args, name, LocationQueryKind::Declaration, full, config)
}

pub(super) fn run_callers_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult> {
    run_call_hierarchy_query(args, name, CallHierarchyDirection::Incoming, config)
}

pub(super) fn run_callees_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult> {
    run_call_hierarchy_query(args, name, CallHierarchyDirection::Outgoing, config)
}

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn with_initialized_client<T, F>(
    path: &Path,
    selected_server: Option<&str>,
    selected_language: Option<&str>,
    detach: bool,
    download: bool,
    wait_for_index_requested: bool,
    debug: bool,
    timeout: Duration,
    config: &ConfigStore,
    run: F,
) -> Result<(PreparedWorkspace, T)>
where
    F: FnOnce(&PreparedWorkspace, &crate::lsp::InitializeResponse, &mut LspClient) -> Result<T>,
{
    let workspace = prepare_workspace(path, selected_server, selected_language, download, config)?;
    let wait_for_index = wait_for_index_requested || workspace.server.wait_for_index;

    let mut client = connect_lsp_client(&workspace, detach, debug, timeout)?;
    let initialize = client
        .initialize(
            &workspace.root_uri,
            &workspace.workspace_name,
            wait_for_index,
        )
        .map_err(|error| {
            error.with_prefix(format!("failed to initialize {}", workspace.server.server))
        })?;

    let response = (if wait_for_index {
        client.wait_for_background_work().map_err(|error| {
            Error::lsp(format!(
                "failed to wait for background work with {}: {error}",
                workspace.server.server
            ))
        })
    } else {
        Ok(())
    })
    .and_then(|()| run(&workspace, &initialize, &mut client));
    let shutdown = client.shutdown();
    let response = response?;
    shutdown.map_err(|error| {
        Error::lsp(format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        ))
    })?;

    Ok((workspace, response))
}

#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
fn with_initialized_client_context<T, C, F>(
    path: &Path,
    selected_server: Option<&str>,
    selected_language: Option<&str>,
    detach: bool,
    download: bool,
    wait_for_index_requested: bool,
    debug: bool,
    timeout: Duration,
    config: &ConfigStore,
    context: C,
    run: F,
) -> Result<(PreparedWorkspace, T)>
where
    F: FnOnce(&PreparedWorkspace, &crate::lsp::InitializeResponse, &mut LspClient, C) -> Result<T>,
{
    with_initialized_client(
        path,
        selected_server,
        selected_language,
        detach,
        download,
        wait_for_index_requested,
        debug,
        timeout,
        config,
        |workspace, initialize, client| run(workspace, initialize, client, context),
    )
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
) -> Result<Vec<SymbolMatch>> {
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
) -> Result<Vec<SymbolMatch>> {
    let files = scan_workspace_files(request.directory, config, workspace)?;
    let mut source_cache = SourceCache::default();
    let matches = client
        .document_symbols_window(
            &files,
            config.cli.max_requests_in_flight(),
            |file, response| {
                let file_matches = if request.function_only {
                    function_matches_from_document_response(response, file, &mut source_cache)?
                } else {
                    document_symbol_matches_from_response(response, file, &mut source_cache)?
                };
                Ok(file_matches
                    .into_iter()
                    .filter(|matched| matched.name == request.name)
                    .collect::<Vec<_>>())
            },
        )
        .map_err(|error| {
            error.with_prefix(format!(
                "failed to discover symbols with {}",
                workspace.server.server
            ))
        })?;

    Ok(dedupe_symbol_matches(
        matches.into_iter().flatten().collect(),
    ))
}

fn fill_definition_full_content(
    workspace: &PreparedWorkspace,
    client: &mut LspClient,
    source_cache: &mut SourceCache,
    matches: &mut [SymbolMatch],
) -> Result<()> {
    let mut responses = std::collections::HashMap::new();

    for matched in matches {
        if !responses.contains_key(&matched.path) {
            let uri = open_document_for(client, &matched.path, &workspace.server.server)?;
            let response = match client.document_symbol(&uri) {
                Ok(response) => Some(response),
                Err(error) if should_skip_document_symbol_error(&error.to_string()) => None,
                Err(_) => None,
            };
            responses.insert(matched.path.clone(), response);
        }

        if let Some(Some(response)) = responses.get(&matched.path) {
            matched.full_content = symbol_full_content_from_document_response(
                response,
                &matched.path,
                matched,
                source_cache,
            )?
            .or_else(|| matched.full_content.clone())
            .or_else(|| Some(matched.line_content.clone()));
        } else if matched.full_content.is_none() {
            matched.full_content = Some(matched.line_content.clone());
        }
    }

    Ok(())
}
