use super::super::{
    Command, DaemonArgs, FormatArgs, LanguagesArgs, RunArgs, ServerCapabilitiesArgs, ServersArgs,
    StopAllArgs, StopArgs, UpdateArgs,
};
use super::{parse, parse_with_config};
use crate::config::{CliConfig, DaemonCliConfig};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn parses_run_arguments() {
    assert_eq!(
        parse(&[
            "run",
            "workspace",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--download",
            "--debug",
        ])
        .expect("run should parse"),
        Command::Run(RunArgs {
            path: PathBuf::from("workspace"),
            lang: Some("rust".to_string()),
            lsp: Some("rust-analyzer".to_string()),
            download: true,
            debug: true,
        })
    );
}

#[test]
fn parses_daemon_arguments_and_config_idle_timeout() {
    assert_eq!(
        parse(&[
            "daemon",
            "workspace",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--download",
            "--debug",
            "--idle-timeout",
            "1.5",
        ])
        .expect("daemon should parse"),
        Command::Daemon(DaemonArgs {
            path: PathBuf::from("workspace"),
            lang: Some("rust".to_string()),
            lsp: Some("rust-analyzer".to_string()),
            download: true,
            debug: true,
            idle_timeout: Duration::from_millis(1500),
        })
    );

    let config = CliConfig {
        daemon: DaemonCliConfig {
            idle_timeout: Some(Duration::from_secs(5)),
        },
        ..CliConfig::default()
    };

    assert_eq!(
        parse_with_config(&["daemon", "workspace"], &config),
        Command::Daemon(DaemonArgs {
            path: PathBuf::from("workspace"),
            lang: None,
            lsp: None,
            download: false,
            debug: false,
            idle_timeout: Duration::from_secs(5),
        })
    );
}

#[test]
fn parses_stop_arguments() {
    assert_eq!(
        parse(&[
            "stop",
            "workspace",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--debug",
        ])
        .expect("stop should parse"),
        Command::Stop(StopArgs {
            path: PathBuf::from("workspace"),
            lang: Some("rust".to_string()),
            lsp: Some("rust-analyzer".to_string()),
            debug: true,
        })
    );
}

#[test]
fn parses_stop_all_arguments_and_debug_default() {
    assert_eq!(
        parse(&["stop-all"]).expect("stop-all should parse"),
        Command::StopAll(StopAllArgs { debug: false })
    );

    let config = CliConfig {
        debug: Some(true),
        ..CliConfig::default()
    };

    assert_eq!(
        parse_with_config(&["stop-all"], &config),
        Command::StopAll(StopAllArgs { debug: true })
    );
}

#[test]
fn parses_languages_arguments() {
    assert_eq!(
        parse(&["languages"]).expect("languages should parse"),
        Command::Languages(LanguagesArgs)
    );
}

#[test]
fn parses_servers_arguments() {
    assert_eq!(
        parse(&["servers"]).expect("servers should parse"),
        Command::Servers(ServersArgs { lang: None })
    );
    assert_eq!(
        parse(&["servers", "--lang", "python"]).expect("servers with --lang should parse"),
        Command::Servers(ServersArgs {
            lang: Some("python".to_string())
        })
    );
}

#[test]
fn parses_server_capabilities_arguments() {
    assert_eq!(
        parse(&[
            "server-capabilities",
            "workspace",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--download",
            "--detach",
            "--debug",
            "--timeout",
            "250ms",
        ])
        .expect("server-capabilities should parse"),
        Command::ServerCapabilities(ServerCapabilitiesArgs {
            directory: PathBuf::from("workspace"),
            lang: Some("rust".to_string()),
            lsp: Some("rust-analyzer".to_string()),
            detach: true,
            download: true,
            debug: true,
            timeout: Duration::from_millis(250),
        })
    );
}

#[test]
fn parses_update_arguments() {
    assert_eq!(
        parse(&["update"]).expect("update should parse"),
        Command::Update(UpdateArgs)
    );
}

#[test]
fn parses_format_arguments() {
    assert_eq!(
        parse(&[
            "fmt",
            "src/main.rs",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--download",
            "--detach",
            "--json",
            "--debug",
            "--timeout",
            "250ms",
            "--check",
        ])
        .expect("format should parse"),
        Command::Format(FormatArgs {
            path: PathBuf::from("src/main.rs"),
            lang: Some("rust".to_string()),
            lsp: Some("rust-analyzer".to_string()),
            download: true,
            detach: true,
            json: true,
            debug: true,
            timeout: Duration::from_millis(250),
            check: true,
            stdout: false,
        })
    );
}

#[test]
fn rejects_format_check_with_stdout() {
    let error = parse(&["format", "src/main.rs", "--check", "--stdout"])
        .expect_err("format should reject --check --stdout");

    assert!(error.contains("`format` does not support using `--check` together with `--stdout`"));
}
