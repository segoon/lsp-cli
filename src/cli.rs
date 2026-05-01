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
    Detect(DetectArgs),
    Grep(GrepArgs),
    ListSymbols(ListSymbolsArgs),
    BuildIndex(BuildIndexArgs),
    Completion(CompletionArgs),
    Run(RunArgs),
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct DetectArgs {
    #[arg(default_value = ".", value_hint = ValueHint::AnyPath)]
    pub path: PathBuf,
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
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct GrepArgs {
    pub pattern: String,
    #[command(flatten)]
    pub query: WorkspaceQueryArgs,
}

#[derive(Debug, Args, Eq, PartialEq)]
pub struct ListSymbolsArgs {
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
mod tests {
    use super::{
        BuildIndexArgs, Command, CompletionArgs, DetectArgs, GrepArgs, ListSymbolsArgs, RunArgs,
        WorkspaceQueryArgs, parse_args,
    };
    use clap_complete::Shell;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn parses_detect_defaults() {
        assert_eq!(
            parse_args(vec!["detect".to_string()]).expect("detect should parse"),
            Command::Detect(DetectArgs {
                path: PathBuf::from("."),
                json: false,
                quiet: false,
                debug: false,
            })
        );
    }

    #[test]
    fn parses_detect_flags_and_path() {
        assert_eq!(
            parse_args(vec![
                "detect".to_string(),
                "src".to_string(),
                "--json".to_string(),
                "-q".to_string(),
            ])
            .expect("detect should parse"),
            Command::Detect(DetectArgs {
                path: PathBuf::from("src"),
                json: true,
                quiet: true,
                debug: false,
            })
        );
    }

    #[test]
    fn parses_grep_arguments() {
        assert_eq!(
            parse_args(vec![
                "grep".to_string(),
                "needle".to_string(),
                "workspace".to_string(),
                "--json".to_string(),
                "--lsp".to_string(),
                "clangd".to_string(),
                "--debug".to_string(),
            ])
            .expect("grep should parse"),
            Command::Grep(GrepArgs {
                pattern: "needle".to_string(),
                query: WorkspaceQueryArgs {
                    directory: PathBuf::from("workspace"),
                    lsp: Some("clangd".to_string()),
                    wait_for_index: false,
                    json: true,
                    debug: true,
                    timeout: Duration::from_secs(10),
                },
            })
        );
    }

    #[test]
    fn parses_grep_timeout_in_seconds_and_milliseconds() {
        assert_eq!(
            parse_args(vec![
                "grep".to_string(),
                "needle".to_string(),
                "workspace".to_string(),
                "--timeout".to_string(),
                "1.5".to_string(),
            ])
            .expect("grep should parse"),
            Command::Grep(GrepArgs {
                pattern: "needle".to_string(),
                query: WorkspaceQueryArgs {
                    directory: PathBuf::from("workspace"),
                    lsp: None,
                    wait_for_index: false,
                    json: false,
                    debug: false,
                    timeout: Duration::from_millis(1500),
                },
            })
        );

        assert_eq!(
            parse_args(vec![
                "grep".to_string(),
                "needle".to_string(),
                "workspace".to_string(),
                "--timeout".to_string(),
                "100ms".to_string(),
            ])
            .expect("grep should parse"),
            Command::Grep(GrepArgs {
                pattern: "needle".to_string(),
                query: WorkspaceQueryArgs {
                    directory: PathBuf::from("workspace"),
                    lsp: None,
                    wait_for_index: false,
                    json: false,
                    debug: false,
                    timeout: Duration::from_millis(100),
                },
            })
        );
    }

    #[test]
    fn rejects_invalid_timeout_value() {
        assert_eq!(
            parse_args(vec![
                "grep".to_string(),
                "needle".to_string(),
                "workspace".to_string(),
                "--timeout".to_string(),
                "nope".to_string(),
            ]),
            Err("error: invalid value 'nope' for '--timeout <T>': invalid timeout \"nope\": expected integer milliseconds or seconds\n\nFor more information, try '--help'.\n".to_string())
        );
    }

    #[test]
    fn rejects_missing_subcommand() {
        let error = parse_args(Vec::<String>::new()).expect_err("missing subcommand should fail");

        assert!(error.contains("Usage: lsp-cli <COMMAND>"));
    }

    #[test]
    fn parses_build_index_arguments() {
        assert_eq!(
            parse_args(vec![
                "build-index".to_string(),
                "workspace".to_string(),
                "--lsp".to_string(),
                "rust-analyzer".to_string(),
                "--debug".to_string(),
                "--timeout".to_string(),
                "500ms".to_string(),
            ])
            .expect("build-index should parse"),
            Command::BuildIndex(BuildIndexArgs {
                directory: PathBuf::from("workspace"),
                lsp: Some("rust-analyzer".to_string()),
                debug: true,
                timeout: Duration::from_millis(500),
            })
        );
    }

    #[test]
    fn parses_completion_arguments() {
        assert_eq!(
            parse_args(vec!["completion".to_string(), "bash".to_string()])
                .expect("completion should parse"),
            Command::Completion(CompletionArgs {
                shell: Some(Shell::Bash),
            })
        );
    }

    #[test]
    fn parses_completion_without_shell() {
        assert_eq!(
            parse_args(vec!["completion".to_string()]).expect("completion should parse"),
            Command::Completion(CompletionArgs { shell: None })
        );
    }

    #[test]
    fn parses_grep_wait_for_index() {
        assert_eq!(
            parse_args(vec![
                "grep".to_string(),
                "needle".to_string(),
                "workspace".to_string(),
                "--wait-for-index".to_string(),
            ])
            .expect("grep should parse"),
            Command::Grep(GrepArgs {
                pattern: "needle".to_string(),
                query: WorkspaceQueryArgs {
                    directory: PathBuf::from("workspace"),
                    lsp: None,
                    wait_for_index: true,
                    json: false,
                    debug: false,
                    timeout: Duration::from_secs(10),
                },
            })
        );
    }

    #[test]
    fn parses_list_symbols_arguments() {
        assert_eq!(
            parse_args(vec![
                "list-symbols".to_string(),
                "workspace".to_string(),
                "--lsp".to_string(),
                "rust-analyzer".to_string(),
                "--json".to_string(),
                "--debug".to_string(),
                "--timeout".to_string(),
                "250ms".to_string(),
            ])
            .expect("list-symbols should parse"),
            Command::ListSymbols(ListSymbolsArgs {
                query: WorkspaceQueryArgs {
                    directory: PathBuf::from("workspace"),
                    lsp: Some("rust-analyzer".to_string()),
                    wait_for_index: false,
                    json: true,
                    debug: true,
                    timeout: Duration::from_millis(250),
                },
            })
        );
    }

    #[test]
    fn parses_list_symbols_wait_for_index() {
        assert_eq!(
            parse_args(vec![
                "list-symbols".to_string(),
                "workspace".to_string(),
                "--wait-for-index".to_string(),
            ])
            .expect("list-symbols should parse"),
            Command::ListSymbols(ListSymbolsArgs {
                query: WorkspaceQueryArgs {
                    directory: PathBuf::from("workspace"),
                    lsp: None,
                    wait_for_index: true,
                    json: false,
                    debug: false,
                    timeout: Duration::from_secs(10),
                },
            })
        );
    }

    #[test]
    fn parses_run_arguments() {
        assert_eq!(
            parse_args(vec![
                "run".to_string(),
                "workspace".to_string(),
                "--lsp".to_string(),
                "rust-analyzer".to_string(),
                "--debug".to_string(),
            ])
            .expect("run should parse"),
            Command::Run(RunArgs {
                path: PathBuf::from("workspace"),
                lsp: Some("rust-analyzer".to_string()),
                debug: true,
            })
        );
    }
}
