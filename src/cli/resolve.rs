use crate::cli::{
    BuildIndexArgs, CatArgs, Command, CommandsArgs, DEFAULT_IDLE_TIMEOUT, DEFAULT_LIMIT,
    DEFAULT_TIMEOUT, DEFAULT_WRITE_STALL_TIMEOUT, DaemonArgs, DetectArgs, DiagnosticsArgs,
    FormatArgs, GrepArgs, InstallDebugArgs, LanguagesArgs, ListFilesArgs, ListFunctionsArgs,
    ListSymbolsArgs, LspWorkspaceQueryArgs, NavigationArgs, RunArgs, SelectionArgs,
    ServerCapabilitiesArgs, ServersArgs, StopAllArgs, StopArgs, UpdateArgs, WorkspaceQueryArgs,
};
use crate::cli::{
    RawBuildIndexArgs, RawCatArgs, RawCommand, RawCommandsArgs, RawDaemonArgs, RawDetectArgs,
    RawDiagnosticsArgs, RawFormatArgs, RawFullNavigationArgs, RawGrepArgs, RawLanguagesArgs,
    RawListFilesArgs, RawListFunctionsArgs, RawListSymbolsArgs, RawLspOptions,
    RawLspWorkspaceQueryArgs, RawNavigationArgs, RawRunArgs, RawServerCapabilitiesArgs,
    RawServersArgs, RawStopAllArgs, RawStopArgs, RawUpdateArgs, RawWorkspaceOptions,
    RawWorkspaceQueryArgs,
};
use crate::config::CliConfig;
use crate::error::{Error, Result};

#[cfg(test)]
use crate::cli::parse_raw_args;

pub(crate) fn resolve_command(command: RawCommand, defaults: &CliConfig) -> Result<Command> {
    let command = match command {
        RawCommand::Commands(_) => Command::Commands(RawCommandsArgs::resolve()),
        RawCommand::Daemon(args) => Command::Daemon(args.resolve(defaults)),
        RawCommand::Stop(args) => Command::Stop(args.resolve(defaults)),
        RawCommand::StopAll(args) => Command::StopAll(args.resolve(defaults)),
        RawCommand::Languages(_) => Command::Languages(RawLanguagesArgs::resolve()),
        RawCommand::Servers(args) => Command::Servers(args.resolve()),
        RawCommand::ServerCapabilities(args) => Command::ServerCapabilities(args.resolve(defaults)),
        RawCommand::Detect(args) => Command::Detect(args.resolve(defaults)),
        RawCommand::Diagnostics(args) => Command::Diagnostics(args.resolve(defaults)),
        RawCommand::Format(args) => Command::Format(args.resolve(defaults)),
        RawCommand::Cat(args) => Command::Cat(args.resolve(defaults)),
        RawCommand::Grep(args) => Command::Grep(args.resolve(defaults)),
        RawCommand::ListSymbols(args) => Command::ListSymbols(args.resolve(defaults)),
        RawCommand::ListFunctions(args) => Command::ListFunctions(args.resolve(defaults)),
        RawCommand::ListFiles(args) => Command::ListFiles(args.resolve(defaults)),
        RawCommand::References(args) => Command::References(args.resolve(defaults)?),
        RawCommand::Callers(args) => Command::Callers(args.resolve(defaults)?),
        RawCommand::Callees(args) => Command::Callees(args.resolve(defaults)?),
        RawCommand::Definition(args) => Command::Definition(args.resolve(defaults)?),
        RawCommand::Declaration(args) => Command::Declaration(args.resolve(defaults)?),
        RawCommand::Implementation(args) => Command::Implementation(args.resolve(defaults)?),
        RawCommand::TypeDefinition(args) => Command::TypeDefinition(args.resolve(defaults)?),
        RawCommand::BuildIndex(args) => Command::BuildIndex(args.resolve(defaults)),
        RawCommand::Update(_) => Command::Update(RawUpdateArgs::resolve()),
        RawCommand::Completion(args) => Command::Completion(args),
        RawCommand::AgentSkill(args) => Command::AgentSkill(args),
        RawCommand::Run(args) => Command::Run(args.resolve(defaults)),
    };
    validate_command(&command)?;
    Ok(command)
}

impl RawCommandsArgs {
    fn resolve() -> CommandsArgs {
        CommandsArgs
    }
}

#[cfg(test)]
pub(crate) fn parse_args<I>(args: I) -> Result<Command>
where
    I: IntoIterator<Item = String>,
{
    parse_raw_args(args).and_then(|command| resolve_command(command, &CliConfig::default()))
}

impl RawDetectArgs {
    fn resolve(self, defaults: &CliConfig) -> DetectArgs {
        DetectArgs {
            path: self.path,
            server: resolve_install_debug_args(self.selector, self.download, self.debug, defaults),
            json: resolve_bool(
                self.json.json,
                self.json.no_json,
                defaults.json.unwrap_or(false),
            ),
            quiet: resolve_bool(
                self.quiet,
                self.no_quiet,
                defaults.detect.quiet.unwrap_or(false),
            ),
        }
    }
}

impl RawWorkspaceQueryArgs {
    fn resolve(self, defaults: &CliConfig) -> WorkspaceQueryArgs {
        self.options.resolve(self.directory, defaults)
    }
}

impl RawWorkspaceOptions {
    fn resolve(self, directory: std::path::PathBuf, defaults: &CliConfig) -> WorkspaceQueryArgs {
        WorkspaceQueryArgs {
            directory,
            selector: resolve_selection_args(self.selector),
            wait_for_index: self.wait_for_index,
            json: resolve_bool(
                self.json.json,
                self.json.no_json,
                defaults.json.unwrap_or(false),
            ),
            debug: resolve_bool(
                self.debug.debug,
                self.debug.no_debug,
                defaults.debug.unwrap_or(false),
            ),
            timeout: self
                .timeout
                .timeout
                .unwrap_or(defaults.timeout.unwrap_or(DEFAULT_TIMEOUT)),
            limit: self
                .limit
                .limit
                .unwrap_or(defaults.limit.unwrap_or(DEFAULT_LIMIT)),
        }
    }
}

impl RawLspWorkspaceQueryArgs {
    fn resolve(self, defaults: &CliConfig) -> LspWorkspaceQueryArgs {
        self.options.resolve(self.query.resolve(defaults), defaults)
    }
}

impl RawLspOptions {
    fn resolve(self, query: WorkspaceQueryArgs, defaults: &CliConfig) -> LspWorkspaceQueryArgs {
        LspWorkspaceQueryArgs {
            query,
            download: resolve_bool(
                self.download.download,
                self.download.no_download,
                defaults.download.unwrap_or(false),
            ),
            detach: resolve_bool(
                self.detach.detach,
                self.detach.no_detach,
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

impl RawCatArgs {
    fn resolve(self, defaults: &CliConfig) -> CatArgs {
        CatArgs {
            path: self.path,
            server: resolve_install_debug_args(self.selector, self.download, self.debug, defaults),
            detach: resolve_bool(
                self.detach.detach,
                self.detach.no_detach,
                defaults.detach.unwrap_or(false),
            ),
            timeout: self
                .timeout
                .timeout
                .unwrap_or(defaults.timeout.unwrap_or(DEFAULT_TIMEOUT)),
        }
    }
}

impl RawFormatArgs {
    fn resolve(self, defaults: &CliConfig) -> FormatArgs {
        FormatArgs {
            path: self.path,
            server: resolve_install_debug_args(self.selector, self.download, self.debug, defaults),
            detach: resolve_bool(
                self.detach.detach,
                self.detach.no_detach,
                defaults.detach.unwrap_or(false),
            ),
            json: resolve_bool(
                self.json.json,
                self.json.no_json,
                defaults.json.unwrap_or(false),
            ),
            timeout: self
                .timeout
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
            server: resolve_install_debug_args(self.selector, self.download, self.debug, defaults),
            detach: resolve_bool(
                self.detach.detach,
                self.detach.no_detach,
                defaults.detach.unwrap_or(false),
            ),
            wait_for_index: self.wait_for_index,
            json: resolve_bool(
                self.json.json,
                self.json.no_json,
                defaults.json.unwrap_or(false),
            ),
            timeout: self
                .timeout
                .timeout
                .unwrap_or(defaults.timeout.unwrap_or(DEFAULT_TIMEOUT)),
            limit: self
                .limit
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

impl RawNavigationArgs {
    fn resolve(self, defaults: &CliConfig) -> Result<NavigationArgs> {
        let target = crate::navigation::NavigationTarget::new(self.name, self.at)?;
        let directory = match &target {
            crate::navigation::NavigationTarget::Name(_) => {
                self.directory.unwrap_or_else(|| ".".into())
            }
            crate::navigation::NavigationTarget::Position { location, .. } => location.path.clone(),
        };
        Ok(NavigationArgs {
            target,
            query: self
                .lsp
                .resolve(self.options.resolve(directory, defaults), defaults),
            full: false,
        })
    }
}

impl RawFullNavigationArgs {
    fn resolve(self, defaults: &CliConfig) -> Result<NavigationArgs> {
        let mut args = self.navigation.resolve(defaults)?;
        args.full = self.full;
        Ok(args)
    }
}

impl RawBuildIndexArgs {
    fn resolve(self, defaults: &CliConfig) -> BuildIndexArgs {
        BuildIndexArgs {
            directory: self.directory,
            server: resolve_install_debug_args(self.selector, self.download, self.debug, defaults),
            detach: resolve_bool(
                self.detach.detach,
                self.detach.no_detach,
                defaults.detach.unwrap_or(false),
            ),
            timeout: self
                .timeout
                .timeout
                .unwrap_or(defaults.timeout.unwrap_or(DEFAULT_TIMEOUT)),
        }
    }
}

impl RawRunArgs {
    fn resolve(self, defaults: &CliConfig) -> RunArgs {
        RunArgs {
            path: self.path,
            server: resolve_install_debug_args(self.selector, self.download, self.debug, defaults),
        }
    }
}

impl RawDaemonArgs {
    fn resolve(self, defaults: &CliConfig) -> DaemonArgs {
        DaemonArgs {
            path: self.path,
            server: resolve_install_debug_args(self.selector, self.download, self.debug, defaults),
            idle_timeout: self
                .idle_timeout
                .unwrap_or(defaults.daemon.idle_timeout.unwrap_or(DEFAULT_IDLE_TIMEOUT)),
            write_stall_timeout: defaults
                .daemon
                .write_stall_timeout
                .unwrap_or(DEFAULT_WRITE_STALL_TIMEOUT),
        }
    }
}

impl RawStopArgs {
    fn resolve(self, defaults: &CliConfig) -> StopArgs {
        StopArgs {
            path: self.path,
            selector: resolve_selection_args(self.selector),
            debug: resolve_bool(
                self.debug.debug,
                self.debug.no_debug,
                defaults.debug.unwrap_or(false),
            ),
        }
    }
}

impl RawStopAllArgs {
    fn resolve(self, defaults: &CliConfig) -> StopAllArgs {
        StopAllArgs {
            debug: resolve_bool(
                self.debug.debug,
                self.debug.no_debug,
                defaults.debug.unwrap_or(false),
            ),
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
            server: resolve_install_debug_args(self.selector, self.download, self.debug, defaults),
            detach: resolve_bool(
                self.detach.detach,
                self.detach.no_detach,
                defaults.detach.unwrap_or(false),
            ),
            json: resolve_bool(
                self.json.json,
                self.json.no_json,
                defaults.json.unwrap_or(false),
            ),
            timeout: self
                .timeout
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

fn resolve_selection_args(selector: crate::cli::RawLangLspArgs) -> SelectionArgs {
    SelectionArgs {
        lang: selector.lang,
        lsp: selector.lsp,
    }
}

fn resolve_install_debug_args(
    selector: crate::cli::RawLangLspArgs,
    download: crate::cli::RawDownloadArgs,
    debug: crate::cli::RawDebugArgs,
    defaults: &CliConfig,
) -> InstallDebugArgs {
    InstallDebugArgs {
        selection: resolve_selection_args(selector),
        download: resolve_bool(
            download.download,
            download.no_download,
            defaults.download.unwrap_or(false),
        ),
        debug: resolve_bool(debug.debug, debug.no_debug, defaults.debug.unwrap_or(false)),
    }
}

fn validate_command(command: &Command) -> Result<()> {
    match command {
        Command::ListFunctions(args) if args.query.files_with_matches => Err(Error::invalid_input(
            "`--files-with-matches` is only supported by grep, references, definition, declaration, implementation, type-definition, callers, and callees",
        )),
        Command::Format(args) if args.check && args.stdout => Err(Error::invalid_input(
            "`format` does not support using `--check` together with `--stdout`",
        )),
        _ => Ok(()),
    }
}
