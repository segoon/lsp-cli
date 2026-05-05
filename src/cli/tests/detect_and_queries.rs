use super::super::{Command, DetectArgs, DiagnosticsArgs, GrepArgs};
use super::{install_debug, lsp_workspace_query, parse, parse_with_config, selection};
use crate::config::{CliConfig, DetectCliConfig};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn parses_detect_defaults() {
    assert_eq!(
        parse(&["detect"]).expect("detect should parse"),
        Command::Detect(DetectArgs {
            path: PathBuf::from("."),
            server: install_debug(None, None, false, false),
            json: false,
            quiet: false,
        })
    );
}

#[test]
fn parses_detect_flags_and_path() {
    assert_eq!(
        parse(&[
            "detect",
            "src",
            "--lang",
            "python",
            "--lsp",
            "pyright",
            "--download",
            "--json",
            "-q",
        ])
        .expect("detect should parse"),
        Command::Detect(DetectArgs {
            path: PathBuf::from("src"),
            server: install_debug(Some("python"), Some("pyright"), true, false),
            json: true,
            quiet: true,
        })
    );
}

#[test]
fn resolves_detect_defaults_from_config_and_no_flags() {
    let config = CliConfig {
        download: Some(true),
        json: Some(true),
        debug: Some(true),
        detect: DetectCliConfig { quiet: Some(true) },
        ..CliConfig::default()
    };

    assert_eq!(
        parse_with_config(&["detect"], &config),
        Command::Detect(DetectArgs {
            path: PathBuf::from("."),
            server: install_debug(None, None, true, true),
            json: true,
            quiet: true,
        })
    );
}

#[test]
fn cli_no_flags_override_boolean_config_defaults() {
    let config = CliConfig {
        download: Some(true),
        json: Some(true),
        debug: Some(true),
        detach: Some(true),
        detect: DetectCliConfig { quiet: Some(true) },
        ..CliConfig::default()
    };

    assert_eq!(
        parse_with_config(
            &[
                "detect",
                "--no-download",
                "--no-json",
                "--no-quiet",
                "--no-debug"
            ],
            &config,
        ),
        Command::Detect(DetectArgs {
            path: PathBuf::from("."),
            server: install_debug(None, None, false, false),
            json: false,
            quiet: false,
        })
    );
}

#[test]
fn parses_grep_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.query.selector = selection(Some("cpp"), Some("clangd"));
    query.download = true;
    query.query.json = true;
    query.query.debug = true;

    assert_eq!(
        parse(&[
            "grep",
            "needle",
            "workspace",
            "--json",
            "--lsp",
            "clangd",
            "--lang",
            "cpp",
            "--download",
            "--debug",
        ])
        .expect("grep should parse"),
        Command::Grep(GrepArgs {
            pattern: "needle".to_string(),
            query,
        })
    );
}

#[test]
fn parses_diagnostics_arguments() {
    let mut query = lsp_workspace_query("workspace");
    query.query.selector = selection(Some("cpp"), Some("clangd"));
    query.download = true;
    query.query.json = true;
    query.query.debug = true;
    query.files_with_matches = true;

    assert_eq!(
        parse(&[
            "diagnostics",
            "workspace",
            "--json",
            "--lsp",
            "clangd",
            "--lang",
            "cpp",
            "--download",
            "--debug",
            "-l",
        ])
        .expect("diagnostics should parse"),
        Command::Diagnostics(DiagnosticsArgs { query })
    );
}

#[test]
fn parses_grep_timeout_in_seconds_and_milliseconds() {
    let mut seconds_query = lsp_workspace_query("workspace");
    seconds_query.query.timeout = Duration::from_millis(1500);

    assert_eq!(
        parse(&["grep", "needle", "workspace", "--timeout", "1.5"]).expect("grep should parse"),
        Command::Grep(GrepArgs {
            pattern: "needle".to_string(),
            query: seconds_query,
        })
    );

    let mut millis_query = lsp_workspace_query("workspace");
    millis_query.query.timeout = Duration::from_millis(100);

    assert_eq!(
        parse(&["grep", "needle", "workspace", "--timeout", "100ms"]).expect("grep should parse"),
        Command::Grep(GrepArgs {
            pattern: "needle".to_string(),
            query: millis_query,
        })
    );
}

#[test]
fn resolves_query_defaults_from_config() {
    let config = CliConfig {
        download: Some(true),
        detach: Some(true),
        json: Some(true),
        debug: Some(true),
        timeout: Some(Duration::from_millis(250)),
        limit: Some(50),
        lsp_preferences: BTreeMap::new(),
        ..CliConfig::default()
    };
    let mut query = lsp_workspace_query("workspace");
    query.download = true;
    query.detach = true;
    query.query.json = true;
    query.query.debug = true;
    query.query.timeout = Duration::from_millis(250);
    query.query.limit = 50;

    assert_eq!(
        parse_with_config(&["grep", "needle", "workspace"], &config),
        Command::Grep(GrepArgs {
            pattern: "needle".to_string(),
            query,
        })
    );
}

#[test]
fn parses_grep_detach() {
    let mut query = lsp_workspace_query("workspace");
    query.detach = true;

    assert_eq!(
        parse(&["grep", "needle", "workspace", "--detach"]).expect("grep should parse"),
        Command::Grep(GrepArgs {
            pattern: "needle".to_string(),
            query,
        })
    );
}

#[test]
fn parses_grep_files_with_matches() {
    let mut query = lsp_workspace_query("workspace");
    query.files_with_matches = true;

    assert_eq!(
        parse(&["grep", "needle", "workspace", "-l"]).expect("grep should parse"),
        Command::Grep(GrepArgs {
            pattern: "needle".to_string(),
            query,
        })
    );
}

#[test]
fn rejects_invalid_timeout_value() {
    assert_eq!(
        parse(&["grep", "needle", "workspace", "--timeout", "nope"]),
        Err("error: invalid value 'nope' for '--timeout <T>': invalid timeout \"nope\": expected integer milliseconds or seconds\n\nFor more information, try '--help'.\n".to_string())
    );
}

#[test]
fn rejects_missing_subcommand() {
    let error = parse(&[]).expect_err("missing subcommand should fail");

    assert!(error.contains("Usage: lsp-cli <COMMAND>"));
}
