mod build_index;
mod callees;
mod callers;
mod common;
mod completion;
mod daemon;
mod diagnostics;
mod declaration;
mod definition;
mod detect;
mod grep;
mod format;
mod languages;
mod list_files;
mod list_functions;
mod list_symbols;
mod references;
mod run;
mod servers;
mod stop;
mod symbol_query;

use crate::cli::{Command as CliCommand, CompletionArgs};
use crate::config::ConfigStore;

pub(crate) fn run(command: CliCommand, config: &ConfigStore) -> Result<String, String> {
    match command {
        CliCommand::Detect(args) => detect::run(&args, config),
        CliCommand::Diagnostics(args) => diagnostics::run(&args, config),
        CliCommand::Format(args) => format::run(&args, config),
        CliCommand::Daemon(args) => daemon::run(&args, config),
        CliCommand::Stop(args) => stop::run(&args, config),
        CliCommand::StopAll(args) => stop::run_all(&args),
        CliCommand::Languages(args) => languages::run(&args, config),
        CliCommand::Servers(args) => servers::run(&args, config),
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
        CliCommand::Completion(_) => unreachable!("completion handled before config loading"),
        CliCommand::Run(args) => run::run(&args, config),
    }
}

pub(crate) fn run_completion(args: CompletionArgs) -> Result<String, String> {
    completion::run(args)
}
