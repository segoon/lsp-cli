use super::kinds::{CallHierarchyDirection, LocationQueryKind};
use super::named::{
    CallHierarchyQueryContext, LocationQueryContext, collect_call_hierarchy_matches,
    collect_location_matches,
};
use super::{WorkspaceSymbolQueryResult, render_workspace_symbol_result, with_initialized_client};
use crate::cli::NavigationArgs;
use crate::config::ConfigStore;
use crate::error::Result;
use crate::lsp::{SymbolMatch, Utf16Position};
use crate::navigation::NavigationTarget;

#[derive(Clone, Copy)]
pub(in crate::commands) enum NavigationKind {
    Location(LocationQueryKind),
    Calls(CallHierarchyDirection),
}

#[derive(Clone)]
pub(super) struct QueryAnchor {
    pub path: std::path::PathBuf,
    pub position: Utf16Position,
    pub name: Option<String>,
    pub kind: Option<lsp_types::SymbolKind>,
}

impl From<SymbolMatch> for QueryAnchor {
    fn from(matched: SymbolMatch) -> Self {
        Self {
            path: matched.path,
            position: matched.position,
            name: matched.name,
            kind: matched.kind,
        }
    }
}

pub(in crate::commands) fn run_navigation(
    args: &NavigationArgs,
    kind: NavigationKind,
    config: &ConfigStore,
) -> Result<String> {
    // Validate the assertion before installing or starting any server.
    let position = match &args.target {
        NavigationTarget::Position {
            location,
            expected_name,
        } => Some(QueryAnchor {
            path: location.path.clone(),
            position: location.resolve(expected_name.as_deref())?,
            name: expected_name.clone(),
            kind: None,
        }),
        NavigationTarget::Name(_) => None,
    };
    let query = &args.query.query;
    let (workspace, matches) = with_initialized_client(
        &query.directory,
        query.server(),
        query.language(),
        args.query.detach,
        args.query.download,
        query.wait_for_index,
        query.debug,
        query.timeout,
        config,
        |workspace, initialize, client| match kind {
            NavigationKind::Location(kind) => collect_location_matches(
                workspace,
                initialize,
                client,
                LocationQueryContext {
                    config,
                    directory: &query.directory,
                    name: args.target.label(),
                    kind,
                    include_full_content: args.full,
                    position: position.as_ref(),
                },
            ),
            NavigationKind::Calls(direction) => collect_call_hierarchy_matches(
                workspace,
                initialize,
                client,
                CallHierarchyQueryContext {
                    config,
                    directory: &query.directory,
                    name: args.target.label(),
                    direction,
                    position: position.as_ref(),
                },
            ),
        },
    )?;
    let result = WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches,
    };
    render_workspace_symbol_result(
        &args.target,
        query,
        args.query.files_with_matches,
        args.full,
        result,
    )
}
