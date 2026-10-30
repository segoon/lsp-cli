use super::super::{Command, CompletionArgs, ListFilesArgs, ListFunctionsArgs};
use super::{
    build_index_args, install_debug, list_symbols_args, lsp_workspace_query, parse, selection,
    workspace_query,
};
use clap_complete::Shell;
use std::time::Duration;

#[test]
fn parses_build_index_arguments() {
    let mut expected = build_index_args("workspace");
    expected.server = install_debug(Some("rust"), Some("rust-analyzer"), true, true);
    expected.timeout = Duration::from_millis(500);

    assert_eq!(
        parse(&[
            "build-index",
            "workspace",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--download",
            "--debug",
            "--timeout",
            "500ms",
        ])
        .expect("build-index should parse"),
        Command::BuildIndex(expected)
    );
}

#[test]
fn parses_completion_arguments() {
    assert_eq!(
        parse(&["completion", "bash"]).expect("completion should parse"),
        Command::Completion(CompletionArgs {
            shell: Some(Shell::Bash),
        })
    );
}

#[test]
fn parses_completion_without_shell() {
    assert_eq!(
        parse(&["completion"]).expect("completion should parse"),
        Command::Completion(CompletionArgs { shell: None })
    );
}

#[test]
fn parses_list_symbols_arguments() {
    let mut expected = list_symbols_args("workspace");
    expected.server = install_debug(Some("rust"), Some("rust-analyzer"), true, true);
    expected.json = true;
    expected.timeout = Duration::from_millis(250);

    assert_eq!(
        parse(&[
            "list-symbols",
            "workspace",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--download",
            "--json",
            "--debug",
            "--timeout",
            "250ms",
        ])
        .expect("list-symbols should parse"),
        Command::ListSymbols(expected)
    );
}

#[test]
fn parses_list_functions_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.query.selector = selection(Some("rust"), Some("rust-analyzer"));
    query.query.json = true;
    query.query.debug = true;
    query.query.timeout = Duration::from_millis(250);

    assert_eq!(
        parse(&[
            "list-functions",
            "workspace",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--json",
            "--debug",
            "--timeout",
            "250ms",
        ])
        .expect("list-functions should parse"),
        Command::ListFunctions(ListFunctionsArgs { query })
    );
}

#[test]
fn parses_list_files_arguments() {
    let mut query = workspace_query("workspace");
    query.selector = selection(Some("rust"), Some("rust-analyzer"));
    query.json = true;
    query.limit = 25;

    assert_eq!(
        parse(&[
            "list-files",
            "workspace",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--json",
            "--limit",
            "25",
        ])
        .expect("list-files should parse"),
        Command::ListFiles(ListFilesArgs { query })
    );
}

#[test]
fn rejects_list_files_detach() {
    let error = parse(&["list-files", "workspace", "--detach"])
        .expect_err("list-files should reject detach");

    assert!(error.contains("unexpected argument '--detach'"));
}

#[test]
fn rejects_files_with_matches_for_list_functions() {
    let error = parse(&["list-functions", "workspace", "-l"]).expect_err("unsupported path output");
    assert!(error.contains("`--files-with-matches` is only supported"));
}
