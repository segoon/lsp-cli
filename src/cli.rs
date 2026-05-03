use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;

use crate::config::{CliConfig, parse_timeout};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_LIMIT: usize = 100;
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Parser)]
#[command(name = "lsp-cli")]
struct RawCli {
    #[command(subcommand)]
    command: RawCommand,
}

#[derive(Debug, Eq, PartialEq, Subcommand)]
pub(crate) enum RawCommand {
    Daemon(RawDaemonArgs),
    Stop(RawStopArgs),
    StopAll(RawStopAllArgs),
    Languages(RawLanguagesArgs),
    Servers(RawServersArgs),
    Detect(RawDetectArgs),
    Grep(RawGrepArgs),
    ListSymbols(RawListSymbolsArgs),
    ListFunctions(RawListFunctionsArgs),
    ListFiles(RawListFilesArgs),
    #[command(alias = "ref")]
    References(RawSymbolQueryArgs),
    Callers(RawSymbolQueryArgs),
    Callees(RawSymbolQueryArgs),
    Definition(RawDefinitionArgs),
    Declaration(RawSymbolQueryArgs),
    BuildIndex(RawBuildIndexArgs),
    Completion(CompletionArgs),
    Run(RawRunArgs),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Daemon(DaemonArgs),
    Stop(StopArgs),
    StopAll(StopAllArgs),
    Languages(LanguagesArgs),
    Servers(ServersArgs),
    Detect(DetectArgs),
    Grep(GrepArgs),
    ListSymbols(ListSymbolsArgs),
    ListFunctions(ListFunctionsArgs),
    ListFiles(ListFilesArgs),
    References(SymbolQueryArgs),
    Callers(SymbolQueryArgs),
    Callees(SymbolQueryArgs),
    Definition(DefinitionArgs),
    Declaration(SymbolQueryArgs),
    BuildIndex(BuildIndexArgs),
    Completion(CompletionArgs),
    Run(RunArgs),
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawDetectArgs {
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    path: PathBuf,
    #[arg(long, conflicts_with = "no_download")]
    download: bool,
    #[arg(long = "no-download", conflicts_with = "download")]
    no_download: bool,
    #[arg(long, conflicts_with = "no_json")]
    json: bool,
    #[arg(long = "no-json", conflicts_with = "json")]
    no_json: bool,
    #[arg(short = 'q', conflicts_with = "no_quiet")]
    quiet: bool,
    #[arg(long = "no-quiet", conflicts_with = "quiet")]
    no_quiet: bool,
    #[arg(long, conflicts_with = "no_debug")]
    debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug")]
    no_debug: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawWorkspaceQueryArgs {
    #[arg(value_hint = ValueHint::DirPath)]
    directory: PathBuf,
    #[arg(long)]
    lang: Option<String>,
    #[arg(long)]
    lsp: Option<String>,
    #[arg(long)]
    wait_for_index: bool,
    #[arg(long, conflicts_with = "no_json")]
    json: bool,
    #[arg(long = "no-json", conflicts_with = "json")]
    no_json: bool,
    #[arg(long, conflicts_with = "no_debug")]
    debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug")]
    no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout)]
    timeout: Option<Duration>,
    #[arg(long, value_name = "N")]
    limit: Option<usize>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawLspWorkspaceQueryArgs {
    #[command(flatten)]
    query: RawWorkspaceQueryArgs,
    #[arg(long, conflicts_with = "no_download")]
    download: bool,
    #[arg(long = "no-download", conflicts_with = "download")]
    no_download: bool,
    #[arg(long, conflicts_with = "no_detach")]
    detach: bool,
    #[arg(long = "no-detach", conflicts_with = "detach")]
    no_detach: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Eq, PartialEq)]
pub struct DetectArgs {
    pub path: PathBuf,
    pub download: bool,
    pub json: bool,
    pub quiet: bool,
    pub debug: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct WorkspaceQueryArgs {
    pub directory: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub wait_for_index: bool,
    pub json: bool,
    pub debug: bool,
    pub timeout: Duration,
    pub limit: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LspWorkspaceQueryArgs {
    pub query: WorkspaceQueryArgs,
    pub download: bool,
    pub detach: bool,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawGrepArgs {
    pattern: String,
    #[command(flatten)]
    query: RawLspWorkspaceQueryArgs,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GrepArgs {
    pub pattern: String,
    pub query: LspWorkspaceQueryArgs,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawListSymbolsArgs {
    #[arg(value_hint = ValueHint::AnyPath)]
    path: PathBuf,
    #[arg(long)]
    lang: Option<String>,
    #[arg(long)]
    lsp: Option<String>,
    #[arg(long, conflicts_with = "no_detach")]
    detach: bool,
    #[arg(long = "no-detach", conflicts_with = "detach")]
    no_detach: bool,
    #[arg(long)]
    wait_for_index: bool,
    #[arg(long, conflicts_with = "no_download")]
    download: bool,
    #[arg(long = "no-download", conflicts_with = "download")]
    no_download: bool,
    #[arg(long, conflicts_with = "no_json")]
    json: bool,
    #[arg(long = "no-json", conflicts_with = "json")]
    no_json: bool,
    #[arg(long, conflicts_with = "no_debug")]
    debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug")]
    no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout)]
    timeout: Option<Duration>,
    #[arg(long, value_name = "N")]
    limit: Option<usize>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Eq, PartialEq)]
pub struct ListSymbolsArgs {
    pub path: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub detach: bool,
    pub wait_for_index: bool,
    pub download: bool,
    pub json: bool,
    pub debug: bool,
    pub timeout: Duration,
    pub limit: usize,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawListFilesArgs {
    #[command(flatten)]
    query: RawWorkspaceQueryArgs,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ListFilesArgs {
    pub query: WorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawListFunctionsArgs {
    #[command(flatten)]
    query: RawLspWorkspaceQueryArgs,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ListFunctionsArgs {
    pub query: LspWorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawSymbolQueryArgs {
    name: String,
    #[command(flatten)]
    query: RawLspWorkspaceQueryArgs,
}

#[derive(Debug, Eq, PartialEq)]
pub struct SymbolQueryArgs {
    pub name: String,
    pub query: LspWorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawDefinitionArgs {
    name: String,
    #[command(flatten)]
    query: RawLspWorkspaceQueryArgs,
    #[arg(long)]
    full: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DefinitionArgs {
    pub name: String,
    pub query: LspWorkspaceQueryArgs,
    pub full: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawBuildIndexArgs {
    #[arg(value_hint = ValueHint::DirPath)]
    directory: PathBuf,
    #[arg(long)]
    lang: Option<String>,
    #[arg(long)]
    lsp: Option<String>,
    #[arg(long, conflicts_with = "no_detach")]
    detach: bool,
    #[arg(long = "no-detach", conflicts_with = "detach")]
    no_detach: bool,
    #[arg(long, conflicts_with = "no_download")]
    download: bool,
    #[arg(long = "no-download", conflicts_with = "download")]
    no_download: bool,
    #[arg(long, conflicts_with = "no_debug")]
    debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug")]
    no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout)]
    timeout: Option<Duration>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct BuildIndexArgs {
    pub directory: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub detach: bool,
    pub download: bool,
    pub debug: bool,
    pub timeout: Duration,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawRunArgs {
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    path: PathBuf,
    #[arg(long)]
    lang: Option<String>,
    #[arg(long)]
    lsp: Option<String>,
    #[arg(long, conflicts_with = "no_download")]
    download: bool,
    #[arg(long = "no-download", conflicts_with = "download")]
    no_download: bool,
    #[arg(long, conflicts_with = "no_debug")]
    debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug")]
    no_debug: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RunArgs {
    pub path: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub download: bool,
    pub debug: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawDaemonArgs {
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    path: PathBuf,
    #[arg(long)]
    lang: Option<String>,
    #[arg(long)]
    lsp: Option<String>,
    #[arg(long, conflicts_with = "no_download")]
    download: bool,
    #[arg(long = "no-download", conflicts_with = "download")]
    no_download: bool,
    #[arg(long, conflicts_with = "no_debug")]
    debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug")]
    no_debug: bool,
    #[arg(long, value_name = "T", value_parser = parse_timeout)]
    idle_timeout: Option<Duration>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DaemonArgs {
    pub path: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub download: bool,
    pub debug: bool,
    pub idle_timeout: Duration,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawStopArgs {
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    path: PathBuf,
    #[arg(long)]
    lang: Option<String>,
    #[arg(long)]
    lsp: Option<String>,
    #[arg(long, conflicts_with = "no_debug")]
    debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug")]
    no_debug: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StopArgs {
    pub path: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub debug: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawStopAllArgs {
    #[arg(long, conflicts_with = "no_debug")]
    debug: bool,
    #[arg(long = "no-debug", conflicts_with = "debug")]
    no_debug: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StopAllArgs {
    pub debug: bool,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawLanguagesArgs {}

#[derive(Debug, Eq, PartialEq)]
pub struct LanguagesArgs;

#[derive(Debug, Args, Eq, PartialEq)]
pub(crate) struct RawServersArgs {
    #[arg(long)]
    lang: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ServersArgs {
    pub lang: Option<String>,
}

#[derive(Clone, Copy, Debug, Args, Eq, PartialEq)]
pub struct CompletionArgs {
    pub shell: Option<Shell>,
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

pub(crate) fn resolve_command(command: RawCommand, defaults: &CliConfig) -> Command {
    match command {
        RawCommand::Daemon(args) => Command::Daemon(args.resolve(defaults)),
        RawCommand::Stop(args) => Command::Stop(args.resolve(defaults)),
        RawCommand::StopAll(args) => Command::StopAll(args.resolve(defaults)),
        RawCommand::Languages(_) => Command::Languages(RawLanguagesArgs::resolve()),
        RawCommand::Servers(args) => Command::Servers(args.resolve()),
        RawCommand::Detect(args) => Command::Detect(args.resolve(defaults)),
        RawCommand::Grep(args) => Command::Grep(args.resolve(defaults)),
        RawCommand::ListSymbols(args) => Command::ListSymbols(args.resolve(defaults)),
        RawCommand::ListFunctions(args) => Command::ListFunctions(args.resolve(defaults)),
        RawCommand::ListFiles(args) => Command::ListFiles(args.resolve(defaults)),
        RawCommand::References(args) => Command::References(args.resolve(defaults)),
        RawCommand::Callers(args) => Command::Callers(args.resolve(defaults)),
        RawCommand::Callees(args) => Command::Callees(args.resolve(defaults)),
        RawCommand::Definition(args) => Command::Definition(args.resolve(defaults)),
        RawCommand::Declaration(args) => Command::Declaration(args.resolve(defaults)),
        RawCommand::BuildIndex(args) => Command::BuildIndex(args.resolve(defaults)),
        RawCommand::Completion(args) => Command::Completion(args),
        RawCommand::Run(args) => Command::Run(args.resolve(defaults)),
    }
}

#[cfg(test)]
pub fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    parse_raw_args(args).map(|command| resolve_command(command, &CliConfig::default()))
}

impl RawDetectArgs {
    fn resolve(self, defaults: &CliConfig) -> DetectArgs {
        DetectArgs {
            path: self.path,
            download: resolve_bool(
                self.download,
                self.no_download,
                defaults.download.unwrap_or(false),
            ),
            json: resolve_bool(self.json, self.no_json, defaults.json.unwrap_or(false)),
            quiet: resolve_bool(
                self.quiet,
                self.no_quiet,
                defaults.detect.quiet.unwrap_or(false),
            ),
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
        }
    }
}

impl RawWorkspaceQueryArgs {
    fn resolve(self, defaults: &CliConfig) -> WorkspaceQueryArgs {
        WorkspaceQueryArgs {
            directory: self.directory,
            lang: self.lang,
            lsp: self.lsp,
            wait_for_index: self.wait_for_index,
            json: resolve_bool(self.json, self.no_json, defaults.json.unwrap_or(false)),
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
            timeout: self
                .timeout
                .unwrap_or(defaults.timeout.unwrap_or(DEFAULT_TIMEOUT)),
            limit: self
                .limit
                .unwrap_or(defaults.limit.unwrap_or(DEFAULT_LIMIT)),
        }
    }
}

impl RawLspWorkspaceQueryArgs {
    fn resolve(self, defaults: &CliConfig) -> LspWorkspaceQueryArgs {
        LspWorkspaceQueryArgs {
            query: self.query.resolve(defaults),
            download: resolve_bool(
                self.download,
                self.no_download,
                defaults.download.unwrap_or(false),
            ),
            detach: resolve_bool(
                self.detach,
                self.no_detach,
                defaults.detach.unwrap_or(false),
            ),
        }
    }
}

impl RawGrepArgs {
    fn resolve(self, defaults: &CliConfig) -> GrepArgs {
        GrepArgs {
            pattern: self.pattern,
            query: self.query.resolve(defaults),
        }
    }
}

impl RawListSymbolsArgs {
    fn resolve(self, defaults: &CliConfig) -> ListSymbolsArgs {
        ListSymbolsArgs {
            path: self.path,
            lang: self.lang,
            lsp: self.lsp,
            detach: resolve_bool(
                self.detach,
                self.no_detach,
                defaults.detach.unwrap_or(false),
            ),
            wait_for_index: self.wait_for_index,
            download: resolve_bool(
                self.download,
                self.no_download,
                defaults.download.unwrap_or(false),
            ),
            json: resolve_bool(self.json, self.no_json, defaults.json.unwrap_or(false)),
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
            timeout: self
                .timeout
                .unwrap_or(defaults.timeout.unwrap_or(DEFAULT_TIMEOUT)),
            limit: self
                .limit
                .unwrap_or(defaults.limit.unwrap_or(DEFAULT_LIMIT)),
        }
    }
}

impl RawListFilesArgs {
    fn resolve(self, defaults: &CliConfig) -> ListFilesArgs {
        ListFilesArgs {
            query: self.query.resolve(defaults),
        }
    }
}

impl RawListFunctionsArgs {
    fn resolve(self, defaults: &CliConfig) -> ListFunctionsArgs {
        ListFunctionsArgs {
            query: self.query.resolve(defaults),
        }
    }
}

impl RawSymbolQueryArgs {
    fn resolve(self, defaults: &CliConfig) -> SymbolQueryArgs {
        SymbolQueryArgs {
            name: self.name,
            query: self.query.resolve(defaults),
        }
    }
}

impl RawDefinitionArgs {
    fn resolve(self, defaults: &CliConfig) -> DefinitionArgs {
        DefinitionArgs {
            name: self.name,
            query: self.query.resolve(defaults),
            full: self.full,
        }
    }
}

impl RawBuildIndexArgs {
    fn resolve(self, defaults: &CliConfig) -> BuildIndexArgs {
        BuildIndexArgs {
            directory: self.directory,
            lang: self.lang,
            lsp: self.lsp,
            detach: resolve_bool(
                self.detach,
                self.no_detach,
                defaults.detach.unwrap_or(false),
            ),
            download: resolve_bool(
                self.download,
                self.no_download,
                defaults.download.unwrap_or(false),
            ),
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
            timeout: self
                .timeout
                .unwrap_or(defaults.timeout.unwrap_or(DEFAULT_TIMEOUT)),
        }
    }
}

impl RawRunArgs {
    fn resolve(self, defaults: &CliConfig) -> RunArgs {
        RunArgs {
            path: self.path,
            lang: self.lang,
            lsp: self.lsp,
            download: resolve_bool(
                self.download,
                self.no_download,
                defaults.download.unwrap_or(false),
            ),
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
        }
    }
}

impl RawDaemonArgs {
    fn resolve(self, defaults: &CliConfig) -> DaemonArgs {
        DaemonArgs {
            path: self.path,
            lang: self.lang,
            lsp: self.lsp,
            download: resolve_bool(
                self.download,
                self.no_download,
                defaults.download.unwrap_or(false),
            ),
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
            idle_timeout: self
                .idle_timeout
                .unwrap_or(defaults.daemon.idle_timeout.unwrap_or(DEFAULT_IDLE_TIMEOUT)),
        }
    }
}

impl RawStopArgs {
    fn resolve(self, defaults: &CliConfig) -> StopArgs {
        StopArgs {
            path: self.path,
            lang: self.lang,
            lsp: self.lsp,
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
        }
    }
}

impl RawStopAllArgs {
    fn resolve(self, defaults: &CliConfig) -> StopAllArgs {
        StopAllArgs {
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
        }
    }
}

impl RawLanguagesArgs {
    fn resolve() -> LanguagesArgs {
        LanguagesArgs
    }
}

impl RawServersArgs {
    fn resolve(self) -> ServersArgs {
        ServersArgs { lang: self.lang }
    }
}

fn resolve_bool(enabled: bool, disabled: bool, default: bool) -> bool {
    if enabled {
        true
    } else if disabled {
        false
    } else {
        default
    }
}

#[cfg(test)]
mod tests;
