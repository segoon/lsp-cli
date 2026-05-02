use super::super::{
    Command, DaemonArgs, LanguagesArgs, RunArgs, ServersArgs, StopAllArgs, StopArgs,
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
