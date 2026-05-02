use crate::cli::BuildIndexArgs;
use crate::commands::common::{connect_lsp_client, prepare_workspace};
use crate::config::ConfigStore;

pub(super) fn run(args: &BuildIndexArgs, config: &ConfigStore) -> Result<String, String> {
    let workspace = prepare_workspace(
        &args.directory,
        args.lsp.as_deref(),
        args.lang.as_deref(),
        args.download,
        config,
    )?;

    let mut client = connect_lsp_client(&workspace, args.detach, args.debug, args.timeout)?;
    client
        .initialize(&workspace.root_uri, &workspace.workspace_name, true)
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;

    let wait = client.wait_for_background_work();
    let shutdown = client.shutdown();
    wait.map_err(|error| {
        format!(
            "failed to build index with {}: {error}",
            workspace.server.server
        )
    })?;
    shutdown.map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;

    Ok(String::new())
}
