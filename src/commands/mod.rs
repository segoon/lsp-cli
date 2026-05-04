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

pub(crate) fn run(command: CliCommand, config: &ConfigStore) -> Result<String, String> {
    match command {
        CliCommand::Detect(args) => detect::run(&args, config),
        CliCommand::Commands(args) => command_list::run(&args),
        CliCommand::Diagnostics(args) => diagnostics::run(&args, config),
        CliCommand::Format(args) => format::run(&args, config),
        CliCommand::Daemon(args) => daemon::run(&args, config),
        CliCommand::Stop(args) => stop::run(&args, config),
        CliCommand::StopAll(args) => stop::run_all(&args),
        CliCommand::Languages(args) => languages::run(&args, config),
        CliCommand::Servers(args) => servers::run(&args, config),
        CliCommand::ServerCapabilities(args) => server_capabilities::run(&args, config),
        CliCommand::Grep(args) => grep::run(&args, config),
        CliCommand::ListSymbols(args) => list_symbols::run(&args, config),
        CliCommand::ListFunctions(args) => list_functions::run(&args, config),
        CliCommand::ListFiles(args) => list_files::run(&args, config),
        CliCommand::References(args) => references::run(&args, config),
        CliCommand::Callers(args) => callers::run(&args, config),
        CliCommand::Callees(args) => callees::run(&args, config),
        CliCommand::Definition(args) => definition::run(&args, config),
        CliCommand::Declaration(args) => declaration::run(&args, config),
        CliCommand::BuildIndex(args) => build_index::run(&args, config),
        CliCommand::Update(args) => update::run(&args, config),
        CliCommand::Completion(_) => unreachable!("completion handled before config loading"),
        CliCommand::AgentSkill(args) => agent_skill::run(&args, config),
        CliCommand::Run(args) => run::run(&args, config),
    }
}

pub(crate) fn run_completion(args: CompletionArgs) -> Result<String, String> {
    completion::run(args)
}

pub(crate) fn run_commands() -> Result<String, String> {
    command_list::run(&crate::cli::CommandsArgs)
}
