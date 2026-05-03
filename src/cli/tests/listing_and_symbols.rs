use super::super::{
    Command, CompletionArgs, DefinitionArgs, ListFilesArgs, ListFunctionsArgs, SymbolQueryArgs,
};
use super::{build_index_args, list_symbols_args, lsp_workspace_query, parse, workspace_query};
use clap_complete::Shell;
use std::time::Duration;

#[test]
fn parses_build_index_arguments() {
    let mut expected = build_index_args("workspace");
    expected.lang = Some("rust".to_string());
    expected.lsp = Some("rust-analyzer".to_string());
    expected.download = true;
    expected.debug = true;
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
    expected.lang = Some("rust".to_string());
    expected.lsp = Some("rust-analyzer".to_string());
    expected.download = true;
    expected.json = true;
    expected.debug = true;
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
    query.query.lang = Some("rust".to_string());
    query.query.lsp = Some("rust-analyzer".to_string());
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
    query.lang = Some("rust".to_string());
    query.lsp = Some("rust-analyzer".to_string());
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
fn parses_references_alias_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.query.lang = Some("rust".to_string());
    query.query.limit = 50;

    assert_eq!(
        parse(&[
            "ref",
            "main",
            "workspace",
            "--lang",
            "rust",
            "--limit",
            "50"
        ])
        .expect("ref should parse"),
        Command::References(SymbolQueryArgs {
            name: "main".to_string(),
            query,
        })
    );
}

#[test]
fn parses_references_files_with_matches_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.files_with_matches = true;

    assert_eq!(
        parse(&["references", "main", "workspace", "--files-with-matches"])
            .expect("references should parse"),
        Command::References(SymbolQueryArgs {
            name: "main".to_string(),
            query,
        })
    );
}

#[test]
fn parses_callers_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.query.lang = Some("rust".to_string());

    assert_eq!(
        parse(&["callers", "main", "workspace", "--lang", "rust"]).expect("callers should parse"),
        Command::Callers(SymbolQueryArgs {
            name: "main".to_string(),
            query,
        })
    );
}

#[test]
fn parses_definition_full_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.query.lang = Some("python".to_string());

    assert_eq!(
        parse(&[
            "definition",
            "Order",
            "workspace",
            "--lang",
            "python",
            "--full"
        ])
        .expect("definition should parse"),
        Command::Definition(DefinitionArgs {
            name: "Order".to_string(),
            query,
            full: true,
        })
    );
}

#[test]
fn parses_definition_files_with_matches_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.files_with_matches = true;

    assert_eq!(
        parse(&["definition", "Order", "workspace", "-l"]).expect("definition should parse"),
        Command::Definition(DefinitionArgs {
            name: "Order".to_string(),
            query,
            full: false,
        })
    );
}

#[test]
fn rejects_full_for_references() {
    let error = parse(&["references", "main", "workspace", "--full"])
        .expect_err("references should reject --full");

    assert!(error.contains("unexpected argument '--full'"));
}

#[test]
fn rejects_files_with_matches_for_list_functions() {
    let error =
        parse(&["list-functions", "workspace", "-l"]).expect_err("list-functions should reject -l");

    assert!(error.contains("`--files-with-matches` is only supported by grep, references, definition, declaration, callers, and callees"));
}

#[test]
fn rejects_full_with_files_with_matches_for_definition() {
    let error = parse(&["definition", "Order", "workspace", "--full", "-l"])
        .expect_err("definition should reject --full -l");

    assert!(error.contains(
        "`definition` does not support using `--full` together with `--files-with-matches`"
    ));
}
