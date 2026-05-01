use super::{
    BuildIndexArgs, Command, CompletionArgs, DaemonArgs, DetectArgs, GrepArgs, ListFilesArgs,
    ListFunctionsArgs, ListSymbolsArgs, RunArgs, SymbolQueryArgs, WorkspaceQueryArgs, parse_args,
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
            download: false,
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
            "--download".to_string(),
            "--json".to_string(),
            "-q".to_string(),
        ])
        .expect("detect should parse"),
        Command::Detect(DetectArgs {
            path: PathBuf::from("src"),
            download: true,
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
                limit: 100,
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
                limit: 100,
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
                limit: 100,
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
                limit: 100,
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
            file: PathBuf::from("workspace"),
            lsp: Some("rust-analyzer".to_string()),
            wait_for_index: false,
            json: true,
            debug: true,
            timeout: Duration::from_millis(250),
            limit: 100,
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
            file: PathBuf::from("workspace"),
            lsp: None,
            wait_for_index: true,
            json: false,
            debug: false,
            timeout: Duration::from_secs(10),
            limit: 100,
        })
    );
}

#[test]
fn parses_list_functions_arguments() {
    assert_eq!(
        parse_args(vec![
            "list-functions".to_string(),
            "workspace".to_string(),
            "--lsp".to_string(),
            "rust-analyzer".to_string(),
            "--json".to_string(),
            "--debug".to_string(),
            "--timeout".to_string(),
            "250ms".to_string(),
        ])
        .expect("list-functions should parse"),
        Command::ListFunctions(ListFunctionsArgs {
            query: WorkspaceQueryArgs {
                directory: PathBuf::from("workspace"),
                lsp: Some("rust-analyzer".to_string()),
                wait_for_index: false,
                json: true,
                debug: true,
                timeout: Duration::from_millis(250),
                limit: 100,
            },
        })
    );
}

#[test]
fn parses_list_functions_wait_for_index() {
    assert_eq!(
        parse_args(vec![
            "list-functions".to_string(),
            "workspace".to_string(),
            "--wait-for-index".to_string(),
        ])
        .expect("list-functions should parse"),
        Command::ListFunctions(ListFunctionsArgs {
            query: WorkspaceQueryArgs {
                directory: PathBuf::from("workspace"),
                lsp: None,
                wait_for_index: true,
                json: false,
                debug: false,
                timeout: Duration::from_secs(10),
                limit: 100,
            },
        })
    );
}

#[test]
fn parses_list_files_arguments() {
    assert_eq!(
        parse_args(vec![
            "list-files".to_string(),
            "workspace".to_string(),
            "--lsp".to_string(),
            "rust-analyzer".to_string(),
            "--json".to_string(),
            "--limit".to_string(),
            "25".to_string(),
        ])
        .expect("list-files should parse"),
        Command::ListFiles(ListFilesArgs {
            query: WorkspaceQueryArgs {
                directory: PathBuf::from("workspace"),
                lsp: Some("rust-analyzer".to_string()),
                wait_for_index: false,
                json: true,
                debug: false,
                timeout: Duration::from_secs(10),
                limit: 25,
            },
        })
    );
}

#[test]
fn parses_references_alias_arguments() {
    assert_eq!(
        parse_args(vec![
            "ref".to_string(),
            "main".to_string(),
            "workspace".to_string(),
            "--limit".to_string(),
            "50".to_string(),
        ])
        .expect("ref should parse"),
        Command::References(SymbolQueryArgs {
            name: "main".to_string(),
            query: WorkspaceQueryArgs {
                directory: PathBuf::from("workspace"),
                lsp: None,
                wait_for_index: false,
                json: false,
                debug: false,
                timeout: Duration::from_secs(10),
                limit: 50,
            },
        })
    );
}

#[test]
fn parses_callers_arguments() {
    assert_eq!(
        parse_args(vec![
            "callers".to_string(),
            "main".to_string(),
            "workspace".to_string(),
        ])
        .expect("callers should parse"),
        Command::Callers(SymbolQueryArgs {
            name: "main".to_string(),
            query: WorkspaceQueryArgs {
                directory: PathBuf::from("workspace"),
                lsp: None,
                wait_for_index: false,
                json: false,
                debug: false,
                timeout: Duration::from_secs(10),
                limit: 100,
            },
        })
    );
}

#[test]
fn parses_callees_arguments() {
    assert_eq!(
        parse_args(vec![
            "callees".to_string(),
            "main".to_string(),
            "workspace".to_string(),
        ])
        .expect("callees should parse"),
        Command::Callees(SymbolQueryArgs {
            name: "main".to_string(),
            query: WorkspaceQueryArgs {
                directory: PathBuf::from("workspace"),
                lsp: None,
                wait_for_index: false,
                json: false,
                debug: false,
                timeout: Duration::from_secs(10),
                limit: 100,
            },
        })
    );
}

#[test]
fn parses_definition_arguments() {
    assert_eq!(
        parse_args(vec![
            "definition".to_string(),
            "main".to_string(),
            "workspace".to_string(),
        ])
        .expect("definition should parse"),
        Command::Definition(SymbolQueryArgs {
            name: "main".to_string(),
            query: WorkspaceQueryArgs {
                directory: PathBuf::from("workspace"),
                lsp: None,
                wait_for_index: false,
                json: false,
                debug: false,
                timeout: Duration::from_secs(10),
                limit: 100,
            },
        })
    );
}

#[test]
fn parses_declaration_arguments() {
    assert_eq!(
        parse_args(vec![
            "declaration".to_string(),
            "main".to_string(),
            "workspace".to_string(),
        ])
        .expect("declaration should parse"),
        Command::Declaration(SymbolQueryArgs {
            name: "main".to_string(),
            query: WorkspaceQueryArgs {
                directory: PathBuf::from("workspace"),
                lsp: None,
                wait_for_index: false,
                json: false,
                debug: false,
                timeout: Duration::from_secs(10),
                limit: 100,
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

#[test]
fn parses_daemon_arguments() {
    assert_eq!(
        parse_args(vec![
            "daemon".to_string(),
            "workspace".to_string(),
            "--lsp".to_string(),
            "rust-analyzer".to_string(),
            "--debug".to_string(),
            "--idle-timeout".to_string(),
            "1.5".to_string(),
        ])
        .expect("daemon should parse"),
        Command::Daemon(DaemonArgs {
            path: PathBuf::from("workspace"),
            lsp: Some("rust-analyzer".to_string()),
            debug: true,
            idle_timeout: Duration::from_millis(1500),
        })
    );
}
