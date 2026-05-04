use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;

use crate::config::parse_timeout;

const HELP_LANG: &str = "Select this language.";
const HELP_LSP: &str = "Use a specific configured LSP server.";
const HELP_DOWNLOAD: &str = "Download LSP server if not found in PATH.";
const HELP_NO_DOWNLOAD: &str = "Do not install missing servers automatically.";
const HELP_JSON: &str = "Print results as JSON.";
const HELP_NO_JSON: &str = "Print human-readable output instead of JSON.";
const HELP_DEBUG: &str = "Print verbose debug logs to stderr.";
const HELP_NO_DEBUG: &str = "Disable verbose debug logs.";
const HELP_TIMEOUT: &str =
    "Per-request LSP timeout. Plain numbers are seconds; values ending in `ms` are milliseconds.";
const HELP_LIMIT: &str = "Maximum number of results to print. Mainly usable for code agents.";
const HELP_WAIT_FOR_INDEX: &str =
    "Wait for background indexing before sending the workspace query.";
const HELP_DETACH: &str = "Use a background daemon socket when available, starting one if needed.";
const HELP_NO_DETACH: &str =
    "Talk to the server in this process instead of using a background daemon.";
const HELP_FILES_WITH_MATCHES: &str = "Print only file paths that contain matches.";

#[derive(Debug, Parser)]
#[command(
    name = "lsp-cli",
    about = "Query language servers from the command line"
)]
struct RawCli {
    #[command(subcommand)]
    command: RawCommand,
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum RawCommand {
    #[command(about = "List canonical top-level subcommands")]
    Commands(RawCommandsArgs),
    #[command(about = "Start a background daemon for the selected workspace and server")]
    Daemon(RawDaemonArgs),
    #[command(about = "Stop the matching background daemon (same cmdline and cwd)")]
    Stop(RawStopArgs),
    #[command(about = "Stop every active lsp-cli daemon (any cmdline, any cwd)")]
    StopAll(RawStopAllArgs),
    #[command(about = "List known languages")]
    Languages(RawLanguagesArgs),
    #[command(about = "List known LSP servers")]
    Servers(RawServersArgs),
    #[command(about = "Show the selected server's advertised capabilities")]
    ServerCapabilities(RawServerCapabilitiesArgs),
    #[command(about = "Detect runnable language servers for a path")]
    Detect(RawDetectArgs),
    #[command(alias = "diag", about = "Print workspace diagnostics")]
    Diagnostics(RawDiagnosticsArgs),
    #[command(alias = "fmt", about = "Format a file")]
    Format(RawFormatArgs),
    #[command(about = "Search workspace symbols (regex syntax is server-dependent)")]
    Grep(RawGrepArgs),
    #[command(about = "List symbols from a file or workspace")]
    ListSymbols(RawListSymbolsArgs),
    #[command(about = "List functions, methods, constructors, and operators")]
    ListFunctions(RawListFunctionsArgs),
    #[command(about = "List files that match the selected workspace filters")]
    ListFiles(RawListFilesArgs),
    #[command(alias = "ref", about = "Find references to a symbol name")]
    References(RawSymbolQueryArgs),
    #[command(about = "Find callers of a symbol name")]
    Callers(RawSymbolQueryArgs),
    #[command(about = "Find callees of a symbol name")]
    Callees(RawSymbolQueryArgs),
    #[command(about = "Find definitions of a symbol name")]
    Definition(RawDefinitionArgs),
    #[command(about = "Find declarations of a symbol name")]
    Declaration(RawDeclarationArgs),
    #[command(about = "Wait for the server to finish indexing a workspace")]
    BuildIndex(RawBuildIndexArgs),
    #[command(about = "Force update langages/servers database")]
    Update(RawUpdateArgs),
    #[command(about = "Generate a shell completion script, write it to stdout")]
    Completion(CompletionArgs),
    #[command(about = "Generate a generic Markdown skill file for code agents")]
    AgentSkill(AgentSkillArgs),
    #[command(about = "Replace lsp-cli with the selected language server process")]
    Run(RawRunArgs),
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawCommandsArgs {}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawDetectArgs {
    #[arg(
        value_name = "PATH",
        default_value = ".",
        value_hint = ValueHint::AnyPath,
        help = "Path to inspect for supported languages and servers."
    )]
    pub(crate) path: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, conflicts_with = "no_download", help = HELP_DOWNLOAD)]
    pub(crate) download: bool,
    #[arg(long = "no-download", conflicts_with = "download", help = HELP_NO_DOWNLOAD)]
    pub(crate) no_download: bool,
    #[arg(long, conflicts_with = "no_json", help = HELP_JSON)]
    pub(crate) json: bool,
    #[arg(long = "no-json", conflicts_with = "json", help = HELP_NO_JSON)]
    pub(crate) no_json: bool,
    #[arg(
        short = 'q',
        conflicts_with = "no_quiet",
        help = "Print only the suggested server command lines."
    )]
    pub(crate) quiet: bool,
    #[arg(
        long = "no-quiet",
        conflicts_with = "quiet",
        help = "Print labeled output instead of only command lines."
    )]
    pub(crate) no_quiet: bool,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawWorkspaceQueryArgs {
    #[arg(
        value_name = "DIRECTORY",
        value_hint = ValueHint::DirPath,
        help = "Workspace directory to query."
    )]
    pub(crate) directory: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, help = HELP_WAIT_FOR_INDEX)]
    pub(crate) wait_for_index: bool,
    #[arg(long, conflicts_with = "no_json", help = HELP_JSON)]
    pub(crate) json: bool,
    #[arg(long = "no-json", conflicts_with = "json", help = HELP_NO_JSON)]
    pub(crate) no_json: bool,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout, help = HELP_TIMEOUT)]
    pub(crate) timeout: Option<Duration>,
    #[arg(long, value_name = "N", help = HELP_LIMIT)]
    pub(crate) limit: Option<usize>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawLspWorkspaceQueryArgs {
    #[command(flatten)]
    pub(crate) query: RawWorkspaceQueryArgs,
    #[arg(long, conflicts_with = "no_download", help = HELP_DOWNLOAD)]
    pub(crate) download: bool,
    #[arg(long = "no-download", conflicts_with = "download", help = HELP_NO_DOWNLOAD)]
    pub(crate) no_download: bool,
    #[arg(long, conflicts_with = "no_detach", help = HELP_DETACH)]
    pub(crate) detach: bool,
    #[arg(long = "no-detach", conflicts_with = "detach", help = HELP_NO_DETACH)]
    pub(crate) no_detach: bool,
    #[arg(short = 'l', long = "files-with-matches", help = HELP_FILES_WITH_MATCHES)]
    pub(crate) files_with_matches: bool,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawGrepArgs {
    #[arg(
        value_name = "PATTERN",
        help = "Pattern to send to `workspace/symbol`."
    )]
    pub(crate) pattern: String,
    #[command(flatten)]
    pub(crate) query: RawLspWorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawDiagnosticsArgs {
    #[command(flatten)]
    pub(crate) query: RawLspWorkspaceQueryArgs,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawFormatArgs {
    #[arg(
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help = "File to format."
    )]
    pub(crate) path: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, conflicts_with = "no_download", help = HELP_DOWNLOAD)]
    pub(crate) download: bool,
    #[arg(long = "no-download", conflicts_with = "download", help = HELP_NO_DOWNLOAD)]
    pub(crate) no_download: bool,
    #[arg(long, conflicts_with = "no_detach", help = HELP_DETACH)]
    pub(crate) detach: bool,
    #[arg(long = "no-detach", conflicts_with = "detach", help = HELP_NO_DETACH)]
    pub(crate) no_detach: bool,
    #[arg(long, conflicts_with = "no_json", help = HELP_JSON)]
    pub(crate) json: bool,
    #[arg(long = "no-json", conflicts_with = "json", help = HELP_NO_JSON)]
    pub(crate) no_json: bool,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout, help = HELP_TIMEOUT)]
    pub(crate) timeout: Option<Duration>,
    #[arg(long, help = "Exit with an error if formatting would change the file.")]
    pub(crate) check: bool,
    #[arg(
        long,
        help = "Write the formatted file to stdout instead of modifying it."
    )]
    pub(crate) stdout: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawListSymbolsArgs {
    #[arg(
        value_name = "PATH",
        value_hint = ValueHint::AnyPath,
        help = "File or directory whose symbols to list."
    )]
    pub(crate) path: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, conflicts_with = "no_detach", help = HELP_DETACH)]
    pub(crate) detach: bool,
    #[arg(long = "no-detach", conflicts_with = "detach", help = HELP_NO_DETACH)]
    pub(crate) no_detach: bool,
    #[arg(long, help = HELP_WAIT_FOR_INDEX)]
    pub(crate) wait_for_index: bool,
    #[arg(long, conflicts_with = "no_download", help = HELP_DOWNLOAD)]
    pub(crate) download: bool,
    #[arg(long = "no-download", conflicts_with = "download", help = HELP_NO_DOWNLOAD)]
    pub(crate) no_download: bool,
    #[arg(long, conflicts_with = "no_json", help = HELP_JSON)]
    pub(crate) json: bool,
    #[arg(long = "no-json", conflicts_with = "json", help = HELP_NO_JSON)]
    pub(crate) no_json: bool,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout, help = HELP_TIMEOUT)]
    pub(crate) timeout: Option<Duration>,
    #[arg(long, value_name = "N", help = HELP_LIMIT)]
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawListFilesArgs {
    #[command(flatten)]
    pub(crate) query: RawWorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawListFunctionsArgs {
    #[command(flatten)]
    pub(crate) query: RawLspWorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawSymbolQueryArgs {
    #[arg(value_name = "NAME", help = "Symbol name to search for.")]
    pub(crate) name: String,
    #[command(flatten)]
    pub(crate) query: RawLspWorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawDefinitionArgs {
    #[arg(value_name = "NAME", help = "Symbol name to search for.")]
    pub(crate) name: String,
    #[command(flatten)]
    pub(crate) query: RawLspWorkspaceQueryArgs,
    #[arg(long, help = "Include full source text for each match in output.")]
    pub(crate) full: bool,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawDeclarationArgs {
    #[arg(value_name = "NAME", help = "Symbol name to search for.")]
    pub(crate) name: String,
    #[command(flatten)]
    pub(crate) query: RawLspWorkspaceQueryArgs,
    #[arg(long, help = "Include full source text for each match in output.")]
    pub(crate) full: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawBuildIndexArgs {
    #[arg(
        value_name = "DIRECTORY",
        value_hint = ValueHint::DirPath,
        help = "Workspace directory to index."
    )]
    pub(crate) directory: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, conflicts_with = "no_detach", help = HELP_DETACH)]
    pub(crate) detach: bool,
    #[arg(long = "no-detach", conflicts_with = "detach", help = HELP_NO_DETACH)]
    pub(crate) no_detach: bool,
    #[arg(long, conflicts_with = "no_download", help = HELP_DOWNLOAD)]
    pub(crate) download: bool,
    #[arg(long = "no-download", conflicts_with = "download", help = HELP_NO_DOWNLOAD)]
    pub(crate) no_download: bool,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout, help = HELP_TIMEOUT)]
    pub(crate) timeout: Option<Duration>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawRunArgs {
    #[arg(
        value_name = "PATH",
        default_value = ".",
        value_hint = ValueHint::AnyPath,
        help = "Path used to detect the workspace and server to run."
    )]
    pub(crate) path: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, conflicts_with = "no_download", help = HELP_DOWNLOAD)]
    pub(crate) download: bool,
    #[arg(long = "no-download", conflicts_with = "download", help = HELP_NO_DOWNLOAD)]
    pub(crate) no_download: bool,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawDaemonArgs {
    #[arg(
        value_name = "PATH",
        default_value = ".",
        value_hint = ValueHint::AnyPath,
        help = "Path used to detect the workspace and server to daemonize."
    )]
    pub(crate) path: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, conflicts_with = "no_download", help = HELP_DOWNLOAD)]
    pub(crate) download: bool,
    #[arg(long = "no-download", conflicts_with = "download", help = HELP_NO_DOWNLOAD)]
    pub(crate) no_download: bool,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
    #[arg(
        long,
        value_name = "T",
        value_parser = parse_timeout,
        help = "Shut the daemon down after this much idle time."
    )]
    pub(crate) idle_timeout: Option<Duration>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawStopArgs {
    #[arg(
        value_name = "PATH",
        default_value = ".",
        value_hint = ValueHint::AnyPath,
        help = "Path used to resolve the daemon to stop."
    )]
    pub(crate) path: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawStopAllArgs {
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawLanguagesArgs {}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawServersArgs {
    #[arg(long, help = "List servers configured for this language only.")]
    pub(crate) lang: Option<String>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawServerCapabilitiesArgs {
    #[arg(
        value_name = "DIRECTORY",
        value_hint = ValueHint::DirPath,
        help = "Workspace directory used to initialize the server."
    )]
    pub(crate) directory: PathBuf,
    #[arg(long, help = HELP_LANG)]
    pub(crate) lang: Option<String>,
    #[arg(long, help = HELP_LSP)]
    pub(crate) lsp: Option<String>,
    #[arg(long, conflicts_with = "no_detach", help = HELP_DETACH)]
    pub(crate) detach: bool,
    #[arg(long = "no-detach", conflicts_with = "detach", help = HELP_NO_DETACH)]
    pub(crate) no_detach: bool,
    #[arg(long, conflicts_with = "no_download", help = HELP_DOWNLOAD)]
    pub(crate) download: bool,
    #[arg(long = "no-download", conflicts_with = "download", help = HELP_NO_DOWNLOAD)]
    pub(crate) no_download: bool,
    #[arg(long, conflicts_with = "no_debug", help = HELP_DEBUG)]
    pub(crate) debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug", help = HELP_NO_DEBUG)]
    pub(crate) no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout, help = HELP_TIMEOUT)]
    pub(crate) timeout: Option<Duration>,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawUpdateArgs {}

#[derive(Clone, Copy, Debug, Args, Eq, PartialEq)]
pub struct CompletionArgs {
    #[arg(
        value_name = "SHELL",
        help = "Shell to generate completion for. Defaults to the current shell from $SHELL."
    )]
    pub shell: Option<Shell>,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct AgentSkillArgs {
    #[arg(
        value_name = "PATH",
        value_hint = ValueHint::FilePath,
        help = "Write the generated skill Markdown to this path. Pass `-` to write to stdout."
    )]
    pub path: PathBuf,
}

pub fn clap_command() -> clap::Command {
    RawCli::command()
}

pub(crate) fn parse_raw_args<I>(args: I) -> Result<RawCommand, String>
where
    I: IntoIterator<Item = String>,
{
    let args = std::iter::once("lsp-cli".to_string()).chain(args);
    RawCli::try_parse_from(args)
        .map(|cli| cli.command)
        .map_err(|error| error.to_string())
}
