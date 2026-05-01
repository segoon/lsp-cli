mod build_index;
mod common;
mod completion;
mod detect;
mod grep;
mod list_functions;
mod list_symbols;
mod run;
mod symbol_query;

use crate::cli::{Command as CliCommand, CompletionArgs};
use crate::config::ConfigStore;

pub(crate) fn run(command: CliCommand, config: &ConfigStore) -> Result<String, String> {
    match command {
        CliCommand::Detect(args) => detect::run(&args, config),
        CliCommand::Grep(args) => grep::run(&args, config),
        CliCommand::ListSymbols(args) => list_symbols::run(&args, config),
        CliCommand::ListFunctions(args) => list_functions::run(&args, config),
        CliCommand::BuildIndex(args) => build_index::run(&args, config),
        CliCommand::Completion(_) => unreachable!("completion handled before config loading"),
        CliCommand::Run(args) => run::run(&args, config),
    }
}

pub(crate) fn run_completion(args: CompletionArgs) -> Result<String, String> {
    completion::run(args)
}
