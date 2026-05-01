use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(name = "lsp-cli")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
pub enum Command {
    Daemon(DaemonArgs),
    Detect(DetectArgs),
    Grep(GrepArgs),
    ListSymbols(ListSymbolsArgs),
    ListFunctions(ListFunctionsArgs),
    ListFiles(ListFilesArgs),
    #[command(alias = "ref")]
    References(SymbolQueryArgs),
    Callers(SymbolQueryArgs),
    Callees(SymbolQueryArgs),
    Definition(SymbolQueryArgs),
    Declaration(SymbolQueryArgs),
    BuildIndex(BuildIndexArgs),
    Completion(CompletionArgs),
    Run(RunArgs),
}

#[derive(Debug, Args, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct DetectArgs {
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    pub path: PathBuf,
    #[arg(long)]
    pub download: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(short = 'q')]
    pub quiet: bool,
    #[arg(long)]
    pub debug: bool,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct WorkspaceQueryArgs {
    #[arg(value_hint = ValueHint::DirPath)]
    pub directory: PathBuf,
    #[arg(long)]
    pub lsp: Option<String>,
    #[arg(long)]
    pub wait_for_index: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub debug: bool,
    #[arg(long, value_name = "T", default_value = "10", value_parser = parse_timeout)]
    pub timeout: Duration,
    #[arg(long, value_name = "N", default_value_t = 100)]
    pub limit: usize,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct GrepArgs {
    pub pattern: String,
    #[command(flatten)]
    pub query: WorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct ListSymbolsArgs {
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,
    #[arg(long)]
    pub lsp: Option<String>,
    #[arg(long)]
    pub wait_for_index: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub debug: bool,
    #[arg(long, value_name = "T", default_value = "10", value_parser = parse_timeout)]
    pub timeout: Duration,
    #[arg(long, value_name = "N", default_value_t = 100)]
    pub limit: usize,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct ListFilesArgs {
    #[command(flatten)]
    pub query: WorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct ListFunctionsArgs {
    #[command(flatten)]
    pub query: WorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct SymbolQueryArgs {
    pub name: String,
    #[command(flatten)]
    pub query: WorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct BuildIndexArgs {
    #[arg(value_hint = ValueHint::DirPath)]
    pub directory: PathBuf,
    #[arg(long)]
    pub lsp: Option<String>,
    #[arg(long)]
    pub debug: bool,
    #[arg(long, value_name = "T", default_value = "10", value_parser = parse_timeout)]
    pub timeout: Duration,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct RunArgs {
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    pub path: PathBuf,
    #[arg(long)]
    pub lsp: Option<String>,
    #[arg(long)]
    pub debug: bool,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct DaemonArgs {
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    pub path: PathBuf,
    #[arg(long)]
    pub lsp: Option<String>,
    #[arg(long)]
    pub debug: bool,
    #[arg(long, value_name = "T", default_value = "60", value_parser = parse_timeout)]
    pub idle_timeout: Duration,
}

#[derive(Clone, Copy, Debug, Args, Eq, PartialEq)]
pub struct CompletionArgs {
    pub shell: Option<Shell>,
}

pub fn clap_command() -> clap::Command {
    Cli::command()
}

pub fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let args = std::iter::once("lsp-cli".to_string()).chain(args);
    Cli::try_parse_from(args)
        .map(|cli| cli.command)
        .map_err(|error| error.to_string())
}

fn parse_timeout(value: &str) -> Result<Duration, String> {
    if let Some(milliseconds) = value.strip_suffix("ms") {
        let milliseconds = milliseconds.parse::<u64>().map_err(|_| {
            format!("invalid timeout {value:?}: expected integer milliseconds or seconds")
        })?;
        return Ok(Duration::from_millis(milliseconds));
    }

    let seconds = value.parse::<f64>().map_err(|_| {
        format!("invalid timeout {value:?}: expected integer milliseconds or seconds")
    })?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(format!(
            "invalid timeout {value:?}: expected non-negative milliseconds or seconds"
        ));
    }

    Ok(Duration::from_secs_f64(seconds))
}

#[cfg(test)]
mod tests;
