mod render;

use crate::cli::CatArgs;
use crate::commands::common::{connect_lsp_client, prepare_workspace};
use crate::config::ConfigStore;
use crate::error::{Error, Result};
use crate::lsp::{path_to_file_uri, semantic_tokens};
use std::io::IsTerminal;

pub(super) fn run(args: &CatArgs, config: &ConfigStore) -> Result<String> {
    let no_color = std::env::var_os("NO_COLOR");
    let term = std::env::var_os("TERM");
    let color = color_enabled(
        std::io::stdout().is_terminal(),
        no_color.as_deref(),
        term.as_deref(),
    );
    run_with_color(args, config, color)
}

fn color_enabled(
    terminal: bool,
    no_color: Option<&std::ffi::OsStr>,
    term: Option<&std::ffi::OsStr>,
) -> bool {
    terminal
        && no_color.is_none_or(std::ffi::OsStr::is_empty)
        && term != Some(std::ffi::OsStr::new("dumb"))
}

fn run_with_color(args: &CatArgs, config: &ConfigStore, color: bool) -> Result<String> {
    let metadata = std::fs::metadata(&args.path).map_err(|error| {
        Error::invalid_input(format!("cat cannot read {}: {error}", args.path.display()))
    })?;
    if !metadata.is_file() {
        return Err(Error::invalid_input(format!(
            "cat expected a regular file, but {} is not a file",
            args.path.display()
        )));
    }
    let source = crate::fs::read_to_string(&args.path)?;
    if !color || source.is_empty() {
        return Ok(source);
    }
    let workspace = prepare_workspace(
        &args.path,
        args.server.server(),
        args.server.language(),
        args.server.download,
        config,
    )?;
    let mut client = connect_lsp_client(&workspace, args.detach, args.server.debug, args.timeout)?;
    let initialize = client
        .initialize(&workspace.root_uri, &workspace.workspace_name, false)
        .map_err(|error| {
            error.with_prefix(format!("failed to initialize {}", workspace.server.server))
        })?;
    let output = highlight_document(&mut client, &args.path, &source, &initialize);
    let shutdown = client.shutdown();
    // Always complete shutdown after a query error, while preserving the original failure.
    let output = output.map_err(|error: Error| {
        error.with_prefix(format!(
            "failed to highlight {} with {}",
            args.path.display(),
            workspace.server.server
        ))
    })?;
    shutdown.map_err(|error| {
        error.with_prefix(format!(
            "failed to stop {} cleanly",
            workspace.server.server
        ))
    })?;
    Ok(output)
}

fn highlight_document(
    client: &mut crate::lsp::LspClient,
    path: &std::path::Path,
    source: &str,
    initialize: &crate::lsp::InitializeResponse,
) -> Result<String> {
    let Some(provider) = &initialize.result.capabilities.semantic_tokens_provider else {
        return Ok(source.to_string());
    };
    let options = semantic_tokens::options(provider);
    if !semantic_tokens::supports_full(options) && options.range != Some(true) {
        return Ok(source.to_string());
    }
    let uri = path_to_file_uri(path)?;
    // Render exactly the snapshot sent to the server, even if the file changes during analysis.
    client.open_document_text(path, &uri, source.to_string())?;
    let response = client.semantic_tokens(&uri, options, source)?;
    render::highlight(source, &response, &options.legend)
}

#[cfg(test)]
mod tests;
