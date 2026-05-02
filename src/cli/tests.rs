use super::{
    BuildIndexArgs, Command, ListSymbolsArgs, LspWorkspaceQueryArgs, WorkspaceQueryArgs,
    parse_args, parse_raw_args, resolve_command,
};
use crate::config::CliConfig;
use std::path::PathBuf;
use std::time::Duration;

mod detect_and_queries;
mod listing_and_symbols;
mod misc_commands;

pub(super) fn workspace_query(directory: &str) -> WorkspaceQueryArgs {
    WorkspaceQueryArgs {
        directory: PathBuf::from(directory),
        lang: None,
        lsp: None,
        wait_for_index: false,
        json: false,
        debug: false,
        timeout: Duration::from_secs(10),
        limit: 100,
    }
}

pub(super) fn lsp_workspace_query(directory: &str) -> LspWorkspaceQueryArgs {
    LspWorkspaceQueryArgs {
        query: workspace_query(directory),
        download: false,
        detach: false,
    }
}

pub(super) fn list_symbols_args(file: &str) -> ListSymbolsArgs {
    ListSymbolsArgs {
        file: PathBuf::from(file),
        lang: None,
        lsp: None,
        detach: false,
        wait_for_index: false,
        download: false,
        json: false,
        debug: false,
        timeout: Duration::from_secs(10),
        limit: 100,
    }
}

pub(super) fn build_index_args(directory: &str) -> BuildIndexArgs {
    BuildIndexArgs {
        directory: PathBuf::from(directory),
        lang: None,
        lsp: None,
        detach: false,
        download: false,
        debug: false,
        timeout: Duration::from_secs(10),
    }
}

pub(super) fn parse(args: &[&str]) -> Result<Command, String> {
    parse_args(raw_args(args))
}

pub(super) fn parse_with_config(args: &[&str], config: &CliConfig) -> Command {
    let raw = parse_raw_args(raw_args(args)).expect("raw parse should succeed");
    resolve_command(raw, config)
}

fn raw_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}
