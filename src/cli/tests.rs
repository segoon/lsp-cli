use super::{
    BuildIndexArgs, Command, InstallDebugArgs, ListSymbolsArgs, LspWorkspaceQueryArgs,
    SelectionArgs, WorkspaceQueryArgs, clap_command, parse_args, parse_raw_args,
    resolve_command,
};
use crate::config::CliConfig;
use crate::error::Result;
use clap::Command as ClapCommand;
use std::path::PathBuf;
use std::time::Duration;

mod detect_and_queries;
mod listing_and_symbols;
mod misc_commands;

pub(super) fn workspace_query(directory: &str) -> WorkspaceQueryArgs {
    WorkspaceQueryArgs {
        directory: PathBuf::from(directory),
        selector: selection(None, None),
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
        files_with_matches: false,
    }
}

pub(super) fn list_symbols_args(path: &str) -> ListSymbolsArgs {
    ListSymbolsArgs {
        path: PathBuf::from(path),
        server: install_debug(None, None, false, false),
        detach: false,
        wait_for_index: false,
        json: false,
        timeout: Duration::from_secs(10),
        limit: 100,
    }
}

pub(super) fn build_index_args(directory: &str) -> BuildIndexArgs {
    BuildIndexArgs {
        directory: PathBuf::from(directory),
        server: install_debug(None, None, false, false),
        detach: false,
        timeout: Duration::from_secs(10),
    }
}

pub(super) fn selection(lang: Option<&str>, lsp: Option<&str>) -> SelectionArgs {
    SelectionArgs {
        lang: lang.map(str::to_string),
        lsp: lsp.map(str::to_string),
    }
}

pub(super) fn install_debug(
    lang: Option<&str>,
    lsp: Option<&str>,
    download: bool,
    debug: bool,
) -> InstallDebugArgs {
    InstallDebugArgs {
        selection: selection(lang, lsp),
        download,
        debug,
    }
}

pub(super) fn parse(args: &[&str]) -> Result<Command> {
    parse_args(raw_args(args))
}

pub(super) fn parse_with_config(args: &[&str], config: &CliConfig) -> Command {
    let raw = parse_raw_args(raw_args(args)).expect("raw parse should succeed");
    resolve_command(raw, config).expect("resolved command should validate")
}

fn raw_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

#[test]
fn every_clap_command_and_argument_has_help() {
    assert_command_help(&clap_command(), "lsp-cli");
}

fn assert_command_help(command: &ClapCommand, path: &str) {
    assert!(
        command.get_about().is_some() || command.get_long_about().is_some(),
        "command `{path}` is missing help text"
    );

    for argument in command.get_arguments() {
        if argument.get_id().as_str() == "help" || argument.is_hide_set() {
            continue;
        }

        assert!(
            argument.get_help().is_some() || argument.get_long_help().is_some(),
            "argument `{path} {}` is missing help text",
            argument.get_id().as_str()
        );
    }

    for subcommand in command.get_subcommands() {
        if subcommand.get_name() == "help" {
            continue;
        }

        let subcommand_path = format!("{path} {}", subcommand.get_name());
        assert_command_help(subcommand, &subcommand_path);
    }
}
