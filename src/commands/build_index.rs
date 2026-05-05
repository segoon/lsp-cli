use crate::cli::BuildIndexArgs;
use crate::commands::common::{connect_lsp_client, prepare_workspace};
use crate::config::ConfigStore;
use crate::error::Result;

pub(super) fn run(args: &BuildIndexArgs, config: &ConfigStore) -> Result<String> {
    let workspace = prepare_workspace(
        &args.directory,
        args.server.server(),
        args.server.language(),
        args.server.download,
        config,
    )?;

    let mut client =
        connect_lsp_client(&workspace, args.detach, args.server.debug, args.timeout)?;
    client
        .initialize(&workspace.root_uri, &workspace.workspace_name, true)
        .map_err(|error| error.with_prefix(format!("failed to initialize {}", workspace.server.server)))?;

    let wait = client.wait_for_background_work();
    let shutdown = client.shutdown();
    wait.map_err(|error| {
        error.with_prefix(format!("failed to build index with {}", workspace.server.server))
    })?;
    shutdown.map_err(|error| {
        error.with_prefix(format!("failed to stop {} cleanly", workspace.server.server))
    })?;

    Ok(String::new())
}
