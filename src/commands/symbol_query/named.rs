#[cfg(test)]
mod tests;

use super::kinds::{CallHierarchyDirection, LocationQueryKind};
use super::{
    NamedAnchorRequest, PreparedWorkspace, dedupe_symbol_matches, fill_definition_full_content,
    open_document_for, select_named_anchors,
};
use crate::config::ConfigStore;
use crate::error::Result;
use crate::lsp::{
    LspClient, SourceCache, SymbolMatch, document_symbol_supported, ensure_workspace_symbol_support,
};
use crate::lsp::{
    ensure_call_hierarchy_support, location_matches_from_response,
    location_matches_from_response_with_full_content, prepare_call_hierarchy_response,
};
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) struct LocationQueryContext<'a> {
    pub(super) config: &'a ConfigStore,
    pub(super) directory: &'a Path,
    pub(super) name: &'a str,
    pub(super) kind: LocationQueryKind,
    pub(super) include_full_content: bool,
    pub(super) position: Option<&'a super::navigation::QueryAnchor>,
}

pub(super) fn collect_location_matches(
    workspace: &PreparedWorkspace,
    initialize: &crate::lsp::InitializeResponse,
    client: &mut LspClient,
    context: LocationQueryContext<'_>,
) -> Result<Vec<SymbolMatch>> {
    context.kind.ensure_support(initialize)?;

    let anchors = resolve_anchors(
        workspace,
        initialize,
        client,
        context.config,
        NamedAnchorRequest {
            directory: context.directory,
            name: context.name,
            function_only: false,
        },
        context.position,
    )?;
    let mut source_cache = SourceCache::default();
    let mut matches = Vec::new();

    for anchor in anchors {
        let uri = open_document_for(client, &anchor.path, &workspace.server.server)?;
        let response = match context.kind.query(client, &uri, anchor.position) {
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
                anchor.name.as_deref(),
                anchor.kind,
                &mut source_cache,
            )?
        } else {
            location_matches_from_response(
                &response,
                anchor.name.as_deref(),
                anchor.kind,
                &mut source_cache,
            )?
        });
    }

    let mut matches = dedupe_symbol_matches(matches);
    if context.include_full_content && document_symbol_supported(initialize) {
        fill_definition_full_content(workspace, client, &mut source_cache, &mut matches)?;
    }

    Ok(matches)
}

#[derive(Clone, Copy)]
pub(super) struct CallHierarchyQueryContext<'a> {
    pub(super) config: &'a ConfigStore,
    pub(super) directory: &'a Path,
    pub(super) name: &'a str,
    pub(super) direction: CallHierarchyDirection,
    pub(super) position: Option<&'a super::navigation::QueryAnchor>,
}

pub(super) fn collect_call_hierarchy_matches(
    workspace: &PreparedWorkspace,
    initialize: &crate::lsp::InitializeResponse,
    client: &mut LspClient,
    context: CallHierarchyQueryContext<'_>,
) -> Result<Vec<SymbolMatch>> {
    ensure_call_hierarchy_support(initialize)?;

    let anchors = resolve_anchors(
        workspace,
        initialize,
        client,
        context.config,
        NamedAnchorRequest {
            directory: context.directory,
            name: context.name,
            function_only: true,
        },
        context.position,
    )?;
    let mut source_cache = SourceCache::default();
    let mut matches = Vec::new();

    for anchor in anchors {
        let uri = open_document_for(client, &anchor.path, &workspace.server.server)?;
        let prepared = client
            .prepare_call_hierarchy(&uri, anchor.position)
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

fn resolve_anchors(
    workspace: &PreparedWorkspace,
    initialize: &crate::lsp::InitializeResponse,
    client: &mut LspClient,
    config: &ConfigStore,
    request: NamedAnchorRequest<'_>,
    position: Option<&super::navigation::QueryAnchor>,
) -> Result<Vec<super::navigation::QueryAnchor>> {
    if let Some(position) = position {
        return Ok(vec![position.clone()]);
    }
    ensure_workspace_symbol_support(initialize)?;
    Ok(
        select_named_anchors(workspace, initialize, client, config, request)?
            .into_iter()
            .map(super::navigation::QueryAnchor::from)
            .collect(),
    )
}
