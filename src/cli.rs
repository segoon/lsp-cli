use std::path::PathBuf;
use std::time::Duration;

mod raw;
mod resolve;

pub use raw::{AgentSkillArgs, CompletionArgs, clap_command};
pub(crate) use raw::{
    RawBuildIndexArgs, RawCommand, RawCommandsArgs, RawDaemonArgs, RawDeclarationArgs,
    RawDefinitionArgs, RawDetectArgs, RawDiagnosticsArgs, RawFormatArgs, RawGrepArgs,
    RawLanguagesArgs, RawListFilesArgs, RawListFunctionsArgs, RawListSymbolsArgs,
    RawLspWorkspaceQueryArgs, RawRunArgs, RawServerCapabilitiesArgs, RawServersArgs,
    RawStopAllArgs, RawStopArgs, RawSymbolQueryArgs, RawUpdateArgs, RawWorkspaceQueryArgs,
    parse_raw_args,
};
pub(crate) use resolve::resolve_command;

#[cfg(test)]
pub(crate) use resolve::parse_args;

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_LIMIT: usize = 100;
pub(crate) const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_mins(1);

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Commands(CommandsArgs),
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
    AgentSkill(AgentSkillArgs),
    Run(RunArgs),
}

#[derive(Debug, Eq, PartialEq)]
pub struct CommandsArgs;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Eq, PartialEq)]
pub struct DetectArgs {
    pub path: PathBuf,
    // Q: langu, lsp, download, debug are duplicated across multiple *Args,
    // is it possible to unify it? e.g. via new common struct
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

#[cfg(test)]
mod tests;
