use std::path::PathBuf;
use std::time::Duration;

mod raw;

pub use raw::{CompletionArgs, clap_command};
pub(crate) use raw::{
    RawBuildIndexArgs, RawCommand, RawDaemonArgs, RawDeclarationArgs, RawDefinitionArgs,
    RawDetectArgs, RawDiagnosticsArgs, RawFormatArgs, RawGrepArgs, RawLanguagesArgs,
    RawListFilesArgs, RawListFunctionsArgs, RawListSymbolsArgs, RawLspWorkspaceQueryArgs,
    RawRunArgs, RawServerCapabilitiesArgs, RawServersArgs, RawStopAllArgs, RawStopArgs,
    RawSymbolQueryArgs, RawUpdateArgs, RawWorkspaceQueryArgs, parse_raw_args,
};

use crate::config::CliConfig;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_LIMIT: usize = 100;
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_mins(1);

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Daemon(DaemonArgs),
    Stop(StopArgs),
    StopAll(StopAllArgs),
    Languages(LanguagesArgs),
    Servers(ServersArgs),
    ServerCapabilities(ServerCapabilitiesArgs),
    Detect(DetectArgs),
    Diagnostics(DiagnosticsArgs),
    Format(FormatArgs),
    Grep(GrepArgs),
    ListSymbols(ListSymbolsArgs),
    ListFunctions(ListFunctionsArgs),
    ListFiles(ListFilesArgs),
    References(SymbolQueryArgs),
    Callers(SymbolQueryArgs),
    Callees(SymbolQueryArgs),
    Definition(DefinitionArgs),
    Declaration(DeclarationArgs),
    BuildIndex(BuildIndexArgs),
    Update(UpdateArgs),
    Completion(CompletionArgs),
    Run(RunArgs),
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Eq, PartialEq)]
pub struct DetectArgs {
    pub path: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
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
    pub files_with_matches: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GrepArgs {
    pub pattern: String,
    pub query: LspWorkspaceQueryArgs,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DiagnosticsArgs {
    pub query: LspWorkspaceQueryArgs,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Eq, PartialEq)]
pub struct FormatArgs {
    pub path: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub download: bool,
    pub detach: bool,
    pub json: bool,
    pub debug: bool,
    pub timeout: Duration,
    pub check: bool,
    pub stdout: bool,
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

#[derive(Debug, Eq, PartialEq)]
pub struct ListFilesArgs {
    pub query: WorkspaceQueryArgs,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ListFunctionsArgs {
    pub query: LspWorkspaceQueryArgs,
}

#[derive(Debug, Eq, PartialEq)]
pub struct SymbolQueryArgs {
    pub name: String,
    pub query: LspWorkspaceQueryArgs,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DefinitionArgs {
    pub name: String,
    pub query: LspWorkspaceQueryArgs,
    pub full: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DeclarationArgs {
    pub name: String,
    pub query: LspWorkspaceQueryArgs,
    pub full: bool,
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

#[derive(Debug, Eq, PartialEq)]
pub struct RunArgs {
    pub path: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub download: bool,
    pub debug: bool,
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

#[derive(Debug, Eq, PartialEq)]
pub struct StopArgs {
    pub path: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub debug: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StopAllArgs {
    pub debug: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LanguagesArgs;

#[derive(Debug, Eq, PartialEq)]
pub struct ServersArgs {
    pub lang: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ServerCapabilitiesArgs {
    pub directory: PathBuf,
    pub lang: Option<String>,
    pub lsp: Option<String>,
    pub detach: bool,
    pub download: bool,
    pub debug: bool,
    pub timeout: Duration,
}

#[derive(Debug, Eq, PartialEq)]
pub struct UpdateArgs;
pub(crate) fn resolve_command(
    command: RawCommand,
    defaults: &CliConfig,
) -> Result<Command, String> {
    let command = match command {
        RawCommand::Daemon(args) => Command::Daemon(args.resolve(defaults)),
        RawCommand::Stop(args) => Command::Stop(args.resolve(defaults)),
        RawCommand::StopAll(args) => Command::StopAll(args.resolve(defaults)),
        RawCommand::Languages(_) => Command::Languages(RawLanguagesArgs::resolve()),
        RawCommand::Servers(args) => Command::Servers(args.resolve()),
        RawCommand::ServerCapabilities(args) => Command::ServerCapabilities(args.resolve(defaults)),
        RawCommand::Detect(args) => Command::Detect(args.resolve(defaults)),
        RawCommand::Diagnostics(args) => Command::Diagnostics(args.resolve(defaults)),
        RawCommand::Format(args) => Command::Format(args.resolve(defaults)),
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
        RawCommand::Update(_) => Command::Update(RawUpdateArgs::resolve()),
        RawCommand::Completion(args) => Command::Completion(args),
        RawCommand::Run(args) => Command::Run(args.resolve(defaults)),
    };
    validate_command(&command)?;
    Ok(command)
}

#[cfg(test)]
pub fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    parse_raw_args(args).and_then(|command| resolve_command(command, &CliConfig::default()))
}

impl RawDetectArgs {
    fn resolve(self, defaults: &CliConfig) -> DetectArgs {
        DetectArgs {
            path: self.path,
            lang: self.lang,
            lsp: self.lsp,
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
            files_with_matches: self.files_with_matches,
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

impl RawDiagnosticsArgs {
    fn resolve(self, defaults: &CliConfig) -> DiagnosticsArgs {
        DiagnosticsArgs {
            query: self.query.resolve(defaults),
        }
    }
}

impl RawFormatArgs {
    fn resolve(self, defaults: &CliConfig) -> FormatArgs {
        FormatArgs {
            path: self.path,
            lang: self.lang,
            lsp: self.lsp,
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
            json: resolve_bool(self.json, self.no_json, defaults.json.unwrap_or(false)),
            debug: resolve_bool(self.debug, self.no_debug, defaults.debug.unwrap_or(false)),
            timeout: self
                .timeout
                .unwrap_or(defaults.timeout.unwrap_or(DEFAULT_TIMEOUT)),
            check: self.check,
            stdout: self.stdout,
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

impl RawDeclarationArgs {
    fn resolve(self, defaults: &CliConfig) -> DeclarationArgs {
        DeclarationArgs {
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

impl RawServerCapabilitiesArgs {
    fn resolve(self, defaults: &CliConfig) -> ServerCapabilitiesArgs {
        ServerCapabilitiesArgs {
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

impl RawUpdateArgs {
    fn resolve() -> UpdateArgs {
        UpdateArgs
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

fn validate_command(command: &Command) -> Result<(), String> {
    match command {
        Command::ListFunctions(args) if args.query.files_with_matches => Err(
            "`--files-with-matches` is only supported by grep, references, definition, declaration, callers, and callees".to_string(),
        ),
        Command::Definition(args) if args.full && args.query.files_with_matches => Err(
            "`definition` does not support using `--full` together with `--files-with-matches`"
                .to_string(),
        ),
        Command::Declaration(args) if args.full && args.query.files_with_matches => Err(
            "`declaration` does not support using `--full` together with `--files-with-matches`"
                .to_string(),
        ),
        Command::Format(args) if args.check && args.stdout => Err(
            "`format` does not support using `--check` together with `--stdout`".to_string(),
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
