use serde_json::Value;

use crate::local_fixture::LocalFixture;
use crate::manifest::{CommandStrategy, Manifest};

#[test]
fn lsp_fixture_command_paths_are_covered() {
    let manifest = Manifest::load_repository().expect("E2E manifest should be valid");
    let fixture = LocalFixture::new().expect("local fixture should initialize");

    for command in manifest.commands_for(CommandStrategy::LspFixture) {
        run_query(&fixture, command);
    }
}

fn run_query(fixture: &LocalFixture, command: &str) {
    let mut args = command_prefix(command);
    args.extend([
        "--lsp".to_string(),
        fixture.server_name().to_string(),
        "--no-download".to_string(),
        "--no-detach".to_string(),
        "--timeout".to_string(),
        "5".to_string(),
    ]);
    if !matches!(command, "server-capabilities" | "format" | "build-index") {
        args.push("--json".to_string());
    }
    if command == "format" {
        args.push("--stdout".to_string());
    }
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = fixture.context().run(&refs);
    output.assert_success();

    match command {
        "server-capabilities" => assert!(output.stdout_text().contains("workspace symbols")),
        "build-index" => assert!(output.stdout_text().is_empty()),
        "format" => {
            assert!(output.stdout_text().contains("    formatted"));
            assert!(
                std::fs::read_to_string(fixture.context().workspace().join("main.fake"))
                    .expect("fixture source should remain readable")
                    .contains("  unformatted")
            );
        }
        "diagnostics" => {
            let value: Value = output.json();
            assert_eq!(
                value
                    .pointer("/diagnostics/0/message")
                    .and_then(Value::as_str),
                Some("synthetic diagnostic")
            );
        }
        other => {
            let value: Value = output.json();
            assert_eq!(
                value.pointer("/matches/0/name").and_then(Value::as_str),
                Some("Target"),
                "command {other}"
            );
        }
    }
}

fn command_prefix(command: &str) -> Vec<String> {
    match command {
        "grep" | "references" | "callers" | "callees" | "definition" | "declaration" => {
            vec![command.to_string(), "Target".to_string(), ".".to_string()]
        }
        "format" => vec![command.to_string(), "main.fake".to_string()],
        "server-capabilities"
        | "diagnostics"
        | "list-symbols"
        | "list-functions"
        | "build-index" => vec![command.to_string(), ".".to_string()],
        other => panic!("LSP fixture strategy has no scenario for {other:?}"),
    }
}
