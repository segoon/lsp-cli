use std::time::Duration;

use crate::cli::LspWorkspaceQueryArgs;
use crate::commands::common::PreparedWorkspace;
use crate::config::ConfigStore;
use crate::error::Result;
use crate::lsp::{
    LspClient, document_symbol_supported, ensure_workspace_symbol_support,
    symbol_matches_from_response,
};

use super::{
    WorkspaceSymbolQueryResult, open_document_for, scan_workspace_files, with_initialized_client,
};

const PRIME_RETRY_ATTEMPTS: u32 = 5;
const PRIME_RETRY_DELAY: Duration = Duration::from_millis(750);

pub(in crate::commands) fn run_workspace_symbol_query(
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
            let matches = client
                .workspace_symbol(query)
                .ok()
                .map(|response| symbol_matches_from_response(&response))
                .transpose()?
                .unwrap_or_default();
            if !matches.is_empty() {
                return Ok(matches);
            }

            prime_workspace_document(&args.query.directory, config, workspace, initialize, client)?;

            // The document-symbol response proves that the server processed the opened document,
            // but not that its workspace index is complete. Keep polling within the existing
            // bound for servers that update workspace symbols asynchronously after that response.
            let mut matches = Vec::new();
            for attempt in 0..PRIME_RETRY_ATTEMPTS {
                if attempt > 0 {
                    std::thread::sleep(PRIME_RETRY_DELAY);
                }
                let response = client.workspace_symbol(query).map_err(|error| {
                    error.with_prefix(format!("failed to query {}", workspace.server.server))
                })?;
                matches = symbol_matches_from_response(&response)?;
                if !matches.is_empty() {
                    break;
                }
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

fn prime_workspace_document(
    directory: &std::path::Path,
    config: &ConfigStore,
    workspace: &PreparedWorkspace,
    initialize: &crate::lsp::InitializeResponse,
    client: &mut LspClient,
) -> Result<()> {
    let files = scan_workspace_files(directory, config, workspace)?;
    let Some(file) = files.first() else {
        return Ok(());
    };
    let uri = open_document_for(client, file, &workspace.server.server)?;
    if document_symbol_supported(initialize) {
        // This request is a best-effort readiness barrier. Some servers advertise document
        // symbols but reject individual files; workspace-symbol polling remains the authoritative
        // operation and will report any subsequent transport failure to the user.
        let _ = client.document_symbol(&uri);
    }
    Ok(())
}
