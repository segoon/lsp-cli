use super::{
    BuildIndexArgs, Command, CompletionArgs, DaemonArgs, DetectArgs, GrepArgs, ListFilesArgs,
    ListFunctionsArgs, ListSymbolsArgs, LspWorkspaceQueryArgs, RunArgs, SymbolQueryArgs,
    WorkspaceQueryArgs, parse_args,
};
use clap_complete::Shell;
use std::path::PathBuf;
use std::time::Duration;

fn workspace_query(directory: &str) -> WorkspaceQueryArgs {
    WorkspaceQueryArgs {
        directory: PathBuf::from(directory),
        lsp: None,
        wait_for_index: false,
        json: false,
        debug: false,
        timeout: Duration::from_secs(10),
        limit: 100,
    }
}

fn lsp_workspace_query(directory: &str) -> LspWorkspaceQueryArgs {
    LspWorkspaceQueryArgs {
        query: workspace_query(directory),
        detach: false,
    }
}

fn list_symbols_args(file: &str) -> ListSymbolsArgs {
    ListSymbolsArgs {
        file: PathBuf::from(file),
        lsp: None,
        detach: false,
        wait_for_index: false,
        json: false,
        debug: false,
        timeout: Duration::from_secs(10),
        limit: 100,
    }
}

fn build_index_args(directory: &str) -> BuildIndexArgs {
    BuildIndexArgs {
        directory: PathBuf::from(directory),
        lsp: None,
        detach: false,
        debug: false,
        timeout: Duration::from_secs(10),
    }
}

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
    let mut query = lsp_workspace_query("workspace");
    query.query.lsp = Some("clangd".to_string());
    query.query.json = true;
    query.query.debug = true;

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
            query,
        })
    );
}

#[test]
fn parses_grep_timeout_in_seconds_and_milliseconds() {
    let mut seconds_query = lsp_workspace_query("workspace");
    seconds_query.query.timeout = Duration::from_millis(1500);

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
            query: seconds_query,
        })
    );

    let mut millis_query = lsp_workspace_query("workspace");
    millis_query.query.timeout = Duration::from_millis(100);

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
            query: millis_query,
        })
    );
}

#[test]
fn parses_grep_detach() {
    let mut query = lsp_workspace_query("workspace");
    query.detach = true;

    assert_eq!(
        parse_args(vec![
            "grep".to_string(),
            "needle".to_string(),
            "workspace".to_string(),
            "--detach".to_string(),
        ])
        .expect("grep should parse"),
        Command::Grep(GrepArgs {
            pattern: "needle".to_string(),
            query,
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
    let mut expected = build_index_args("workspace");
    expected.lsp = Some("rust-analyzer".to_string());
    expected.debug = true;
    expected.timeout = Duration::from_millis(500);

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
        Command::BuildIndex(expected)
    );
}

#[test]
fn parses_build_index_detach() {
    let mut expected = build_index_args("workspace");
    expected.detach = true;

    assert_eq!(
        parse_args(vec![
            "build-index".to_string(),
            "workspace".to_string(),
            "--detach".to_string(),
        ])
        .expect("build-index should parse"),
        Command::BuildIndex(expected)
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
    let mut query = lsp_workspace_query("workspace");
    query.query.wait_for_index = true;

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
            query,
        })
    );
}

#[test]
fn parses_list_symbols_arguments() {
    let mut expected = list_symbols_args("workspace");
    expected.lsp = Some("rust-analyzer".to_string());
    expected.json = true;
    expected.debug = true;
    expected.timeout = Duration::from_millis(250);

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
        Command::ListSymbols(expected)
    );
}

#[test]
fn parses_list_symbols_wait_for_index() {
    let mut expected = list_symbols_args("workspace");
    expected.wait_for_index = true;

    assert_eq!(
        parse_args(vec![
            "list-symbols".to_string(),
            "workspace".to_string(),
            "--wait-for-index".to_string(),
        ])
        .expect("list-symbols should parse"),
        Command::ListSymbols(expected)
    );
}

#[test]
fn parses_list_symbols_detach() {
    let mut expected = list_symbols_args("workspace");
    expected.detach = true;

    assert_eq!(
        parse_args(vec![
            "list-symbols".to_string(),
            "workspace".to_string(),
            "--detach".to_string(),
        ])
        .expect("list-symbols should parse"),
        Command::ListSymbols(expected)
    );
}

#[test]
fn parses_list_functions_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.query.lsp = Some("rust-analyzer".to_string());
    query.query.json = true;
    query.query.debug = true;
    query.query.timeout = Duration::from_millis(250);

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
        Command::ListFunctions(ListFunctionsArgs { query })
    );
}

#[test]
fn parses_list_functions_wait_for_index() {
    let mut query = lsp_workspace_query("workspace");
    query.query.wait_for_index = true;

    assert_eq!(
        parse_args(vec![
            "list-functions".to_string(),
            "workspace".to_string(),
            "--wait-for-index".to_string(),
        ])
        .expect("list-functions should parse"),
        Command::ListFunctions(ListFunctionsArgs { query })
    );
}

#[test]
fn parses_list_files_arguments() {
    let mut query = workspace_query("workspace");
    query.lsp = Some("rust-analyzer".to_string());
    query.json = true;
    query.limit = 25;

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
        Command::ListFiles(ListFilesArgs { query })
    );
}

#[test]
fn rejects_list_files_detach() {
    let error = parse_args(vec![
        "list-files".to_string(),
        "workspace".to_string(),
        "--detach".to_string(),
    ])
    .expect_err("list-files should reject detach");

    assert!(error.contains("unexpected argument '--detach'"));
}

#[test]
fn parses_references_alias_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.query.limit = 50;

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
            query,
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
            query: lsp_workspace_query("workspace"),
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
            query: lsp_workspace_query("workspace"),
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
            query: lsp_workspace_query("workspace"),
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
            query: lsp_workspace_query("workspace"),
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
