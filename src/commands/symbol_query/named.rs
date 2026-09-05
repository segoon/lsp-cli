#[cfg(test)]
mod tests;

use super::kinds::{CallHierarchyDirection, LocationQueryKind};
use super::kinds::{zero_based_col, zero_based_line};
use super::{
    NamedAnchorRequest, PreparedWorkspace, WorkspaceSymbolQueryResult, dedupe_symbol_matches,
    fill_definition_full_content, open_document_for, select_named_anchors,
    with_initialized_client_context,
};
use crate::cli::LspWorkspaceQueryArgs;
use crate::config::ConfigStore;
use crate::error::{Error, Result};
use crate::lsp::{
    LspClient, SourceCache, SymbolMatch, document_symbol_supported,
    ensure_workspace_symbol_support, symbol_matches_from_response,
};
use crate::lsp::{
    ensure_call_hierarchy_support, location_matches_from_response,
    location_matches_from_response_with_full_content, prepare_call_hierarchy_response,
};
use std::path::Path;

#[derive(Clone, Copy)]
struct NamedLocationQueryContext<'a> {
    config: &'a ConfigStore,
    directory: &'a Path,
    name: &'a str,
    kind: LocationQueryKind,
    include_full_content: bool,
}

fn collect_named_location_matches(
    workspace: &PreparedWorkspace,
    initialize: &crate::lsp::InitializeResponse,
    client: &mut LspClient,
    context: NamedLocationQueryContext<'_>,
) -> Result<Vec<SymbolMatch>> {
    ensure_workspace_symbol_support(initialize)?;
    context.kind.ensure_support(initialize)?;

    let anchors = client.workspace_symbol(context.name).map_err(|error| {
        Error::lsp(format!(
            "failed to find matching symbols for {:?} with {}: {error}",
            context.name, workspace.server.server
        ))
    })?;
    let workspace_anchors = symbol_matches_from_response(&anchors)?;
    let anchors = select_named_anchors(
        workspace,
        initialize,
        client,
        context.config,
        NamedAnchorRequest {
            directory: context.directory,
            name: context.name,
            function_only: false,
        },
        workspace_anchors,
    )?;
    let mut source_cache = SourceCache::default();
    let mut matches = Vec::new();

    for anchor in anchors {
        let uri = open_document_for(client, &anchor.path, &workspace.server.server)?;
        // QD: avoid using Option::map_err()
        // A: The code was using `Result::map_err()`, not `Option::map_err()`.
        // A: I still applied the style request and rewrote it with explicit
        // A: control flow so the failure branch is easier to read.
        let response = match context.kind.query(client, &uri, &anchor) {
            Ok(response) => response,
            Err(error) => {
                return Err(error.with_prefix(format!(
                    "failed to query {} for {} of {:?}",
                    workspace.server.server,
                    context.kind.label(),
                    context.name
                )));
            }
        };
        matches.extend(if context.include_full_content {
            location_matches_from_response_with_full_content(
                &response,
                &anchor.name,
                anchor.kind,
                &mut source_cache,
            )?
        } else {
            location_matches_from_response(&response, &anchor.name, anchor.kind, &mut source_cache)?
        });
    }

    let mut matches = dedupe_symbol_matches(matches);
    if context.include_full_content && document_symbol_supported(initialize) {
        fill_definition_full_content(workspace, client, &mut source_cache, &mut matches)?;
    }

    Ok(matches)
}

#[derive(Clone, Copy)]
struct CallHierarchyQueryContext<'a> {
    config: &'a ConfigStore,
    directory: &'a Path,
    name: &'a str,
    direction: CallHierarchyDirection,
}

fn collect_call_hierarchy_matches(
    workspace: &PreparedWorkspace,
    initialize: &crate::lsp::InitializeResponse,
    client: &mut LspClient,
    context: CallHierarchyQueryContext<'_>,
) -> Result<Vec<SymbolMatch>> {
    ensure_workspace_symbol_support(initialize)?;
    ensure_call_hierarchy_support(initialize)?;

    let anchors = client.workspace_symbol(context.name).map_err(|error| {
        Error::lsp(format!(
            "failed to find matching symbols for {:?} with {}: {error}",
            context.name, workspace.server.server
        ))
    })?;
    let workspace_anchors = symbol_matches_from_response(&anchors)?;
    let anchors = select_named_anchors(
        workspace,
        initialize,
        client,
        context.config,
        NamedAnchorRequest {
            directory: context.directory,
            name: context.name,
            function_only: true,
        },
        workspace_anchors,
    )?;
    let mut source_cache = SourceCache::default();
    let mut matches = Vec::new();

    for anchor in anchors {
        let uri = open_document_for(client, &anchor.path, &workspace.server.server)?;
        let prepared = client
            .prepare_call_hierarchy(&uri, zero_based_line(&anchor), zero_based_col(&anchor))
            .map_err(|error| {
                error.with_prefix(format!(
                    "failed to prepare call hierarchy with {} for {:?}",
                    workspace.server.server, context.name
                ))
            })?;
        let items = prepare_call_hierarchy_response(&prepared)?;

        for item in &items {
            let response = context.direction.query(client, item).map_err(|error| {
                error.with_prefix(format!(
                    "failed to query {} for {} of {:?}",
                    workspace.server.server,
                    context.direction.label(),
                    context.name
                ))
            })?;
            matches.extend(context.direction.decode(&response, &mut source_cache)?);
        }
    }

    Ok(dedupe_symbol_matches(matches))
}

pub(super) fn run_named_location_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    kind: LocationQueryKind,
    include_full_content: bool,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult> {
    let (workspace, matches) = with_initialized_client_context(
        &args.query.directory,
        args.query.selector.selected_server(),
        args.query.selector.selected_language(),
        args.detach,
        args.download,
        args.query.wait_for_index,
        args.query.debug,
        args.query.timeout,
        config,
        NamedLocationQueryContext {
            config,
            directory: &args.query.directory,
            name,
            kind,
            include_full_content,
        },
        collect_named_location_matches,
    )?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}

pub(super) fn run_call_hierarchy_query(
    args: &LspWorkspaceQueryArgs,
    name: &str,
    direction: CallHierarchyDirection,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult> {
    let query = &args.query;
    let (workspace, matches) = with_initialized_client_context(
        &query.directory,
        query.selector.selected_server(),
        query.selector.selected_language(),
        args.detach,
        args.download,
        query.wait_for_index,
        query.debug,
        query.timeout,
        config,
        CallHierarchyQueryContext {
            config,
            directory: &query.directory,
            name,
            direction,
        },
        collect_call_hierarchy_matches,
    )?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    })
}
