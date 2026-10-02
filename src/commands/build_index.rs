use crate::cli::BuildIndexArgs;
use crate::commands::common::{connect_lsp_client, prepare_workspace};
use crate::config::{BuildIndexCompletion, ConfigStore};
use crate::error::Result;

pub(super) fn run(args: &BuildIndexArgs, config: &ConfigStore) -> Result<String> {
    let selected = &args.server;
    let workspace = prepare_workspace(
        &args.directory,
        selected.server(),
        selected.language(),
        selected.download,
        config,
    )?;

    let mut client = connect_lsp_client(&workspace, args.detach, selected.debug, args.timeout)?;
    client
        .initialize(&workspace.root_uri, &workspace.workspace_name, true)
        .map_err(|error| {
            error.with_prefix(format!("failed to initialize {}", workspace.server.server))
        })?;

    let wait = match workspace.server.build_index_completion {
        BuildIndexCompletion::Confirmed => client.wait_for_background_work(),
        BuildIndexCompletion::BestEffort => client.wait_for_background_work_best_effort(),
    };
    let shutdown = client.shutdown();
    wait.map_err(|error| {
        error.with_prefix(format!(
            "failed to build index with {}",
            workspace.server.server
        ))
    })?;
    shutdown.map_err(|error| {
        error.with_prefix(format!(
            "failed to stop {} cleanly",
            workspace.server.server
        ))
    })?;

    Ok(String::new())
}
