mod agent_skill;
mod build_index;
mod callees;
mod callers;
mod command_list;
mod common;
mod completion;
mod daemon;
mod declaration;
mod definition;
mod detect;
mod diagnostics;
mod format;
mod grep;
mod languages;
mod list_files;
mod list_functions;
mod list_symbols;
mod references;
mod run;
mod server_capabilities;
mod servers;
mod stop;
mod symbol_query;
mod update;

use crate::cli::{Command as CliCommand, CompletionArgs};
use crate::config::ConfigStore;
use crate::error::{Error, Result};

pub(crate) fn run(command: CliCommand, config: &ConfigStore) -> Result<String> {
    match command {
        CliCommand::Detect(args) => detect::run(&args, config),
        CliCommand::Commands(args) => command_list::run(&args).map_err(Error::unexpected),
        CliCommand::Diagnostics(args) => {
            diagnostics::run(&args, config).map_err(Error::from_query_message)
        }
        CliCommand::Format(args) => format::run(&args, config),
        CliCommand::Daemon(args) => daemon::run(&args, config).map_err(Error::from_query_message),
        CliCommand::Stop(args) => stop::run(&args, config).map_err(Error::from_query_message),
        CliCommand::StopAll(args) => stop::run_all(&args).map_err(Error::unexpected),
        CliCommand::Languages(args) => languages::run(&args, config).map_err(Error::unexpected),
        CliCommand::Servers(args) => servers::run(&args, config).map_err(Error::invalid_input),
        CliCommand::ServerCapabilities(args) => {
            server_capabilities::run(&args, config).map_err(Error::from_query_message)
        }
        CliCommand::Grep(args) => grep::run(&args, config).map_err(Error::from_query_message),
        CliCommand::ListSymbols(args) => {
            list_symbols::run(&args, config).map_err(Error::from_query_message)
        }
        CliCommand::ListFunctions(args) => {
            list_functions::run(&args, config).map_err(Error::from_query_message)
        }
        CliCommand::ListFiles(args) => list_files::run(&args, config).map_err(Error::from_query_message),
        CliCommand::References(args) => {
            references::run(&args, config).map_err(Error::from_query_message)
        }
        CliCommand::Callers(args) => callers::run(&args, config).map_err(Error::from_query_message),
        CliCommand::Callees(args) => callees::run(&args, config).map_err(Error::from_query_message),
        CliCommand::Definition(args) => {
            definition::run(&args, config).map_err(Error::from_query_message)
        }
        CliCommand::Declaration(args) => {
            declaration::run(&args, config).map_err(Error::from_query_message)
        }
        CliCommand::BuildIndex(args) => {
            build_index::run(&args, config).map_err(Error::from_query_message)
        }
        CliCommand::Update(args) => update::run(&args, config),
        CliCommand::Completion(_) => unreachable!("completion handled before config loading"),
        CliCommand::AgentSkill(args) => agent_skill::run(&args).map_err(Error::unexpected),
        CliCommand::Run(args) => run::run(&args, config),
    }
}

pub(crate) fn run_completion(args: CompletionArgs) -> Result<String> {
    completion::run(args).map_err(Error::invalid_input)
}

pub(crate) fn run_commands() -> Result<String> {
    command_list::run(&crate::cli::CommandsArgs).map_err(Error::unexpected)
}
