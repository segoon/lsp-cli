mod agent_skill;
mod build_index;
mod command_list;
mod common;
mod completion;
mod daemon;
mod detect;
mod diagnostics;
mod format;
mod grep;
mod languages;
mod list_files;
mod list_functions;
mod list_symbols;
mod run;
mod server_capabilities;
mod servers;
mod stop;
mod symbol_query;
mod update;

use crate::cli::{Command as CliCommand, CompletionArgs};
use crate::config::ConfigStore;
use crate::error::{Error, Result};
use symbol_query::kinds::{CallHierarchyDirection as Calls, LocationQueryKind as Location};
use symbol_query::{NavigationKind, run_navigation};

pub(crate) fn run(command: CliCommand, config: &ConfigStore) -> Result<String> {
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
        CliCommand::References(args) => run_navigation(
            &args,
            NavigationKind::Location(Location::References),
            config,
        ),
        CliCommand::Callers(args) => {
            run_navigation(&args, NavigationKind::Calls(Calls::Incoming), config)
        }
        CliCommand::Callees(args) => {
            run_navigation(&args, NavigationKind::Calls(Calls::Outgoing), config)
        }
        CliCommand::Definition(args) => run_navigation(
            &args,
            NavigationKind::Location(Location::Definition),
            config,
        ),
        CliCommand::Declaration(args) => run_navigation(
            &args,
            NavigationKind::Location(Location::Declaration),
            config,
        ),
        CliCommand::Implementation(args) => run_navigation(
            &args,
            NavigationKind::Location(Location::Implementation),
            config,
        ),
        CliCommand::TypeDefinition(args) => run_navigation(
            &args,
            NavigationKind::Location(Location::TypeDefinition),
            config,
        ),
        CliCommand::BuildIndex(args) => build_index::run(&args, config),
        CliCommand::Update(args) => update::run(&args, config),
        CliCommand::Completion(_) => Err(Error::unexpected(
            "completion command should be dispatched before config loading",
        )),
        CliCommand::AgentSkill(args) => agent_skill::run(&args),
        CliCommand::Run(args) => run::run(&args, config),
    }
}

pub(crate) fn run_completion(args: CompletionArgs) -> Result<String> {
    completion::run(args)
}

pub(crate) fn run_commands() -> Result<String> {
    command_list::run(&crate::cli::CommandsArgs)
}
