use serde_json::Value;

use super::local_fixture::LocalFixture;
use crate::manifest::CommandStrategy;

pub fn lsp_fixture_command_paths_are_covered() {
    let fixture = LocalFixture::new().expect("local fixture should initialize");

    for command in fixture.commands_for(CommandStrategy::LspFixture) {
        run_query(&fixture, command, None);
        if matches!(
            command,
            "definition"
                | "declaration"
                | "implementation"
                | "type-definition"
                | "references"
                | "callers"
                | "callees"
        ) {
            for assertion in [false, true] {
                run_query(&fixture, command, Some(assertion));
            }
        }
    }
    let output = fixture.context().run(&[
        "definition",
        "--at",
        "main.fake:1:4",
        "Wrong",
        "--lsp",
        "does-not-exist",
    ]);
    assert!(output.ensure_success().is_err());
    assert!(
        output
            .stderr_text()
            .contains("expected \"Wrong\" starting at")
    );
}

fn run_query(fixture: &LocalFixture, command: &str, assertion: Option<bool>) {
    let mut args = if let Some(assertion) = assertion {
        let mut args = vec![command.to_string(), "--at".into(), "main.fake:1:4".into()];
        if assertion {
            args.push("Target".into());
        }
        args
    } else {
        command_prefix(command)
    };
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
            let expected_name =
                if assertion == Some(false) && !matches!(command, "callers" | "callees") {
                    None
                } else {
                    Some("Target")
                };
            assert_eq!(
                value.pointer("/matches/0/name").and_then(Value::as_str),
                expected_name,
                "command {other}"
            );
            if let Some(assertion) = assertion {
                assert_eq!(value.get("at").expect("position metadata"), "main.fake:1:4");
                assert_eq!(
                    value.get("expected_name").and_then(Value::as_str),
                    assertion.then_some("Target")
                );
                if !matches!(command, "callers" | "callees") {
                    assert!(value.pointer("/matches/0/kind").is_some_and(Value::is_null));
                }
            }
        }
    }
}

fn command_prefix(command: &str) -> Vec<String> {
    match command {
        "grep" | "references" | "callers" | "callees" | "definition" | "declaration"
        | "implementation" | "type-definition" => {
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

pub fn unadvertised_capabilities_produce_user_facing_errors() {
    let fixture = LocalFixture::new_unsupported().expect("local fixture should initialize");

    for (command, expected) in [
        ("grep", "does not support workspace/symbol"),
        ("format", "does not support format"),
    ] {
        let mut args = command_prefix(command);
        args.extend([
            "--lsp".to_string(),
            fixture.unsupported_server_name().to_string(),
            "--no-download".to_string(),
            "--no-detach".to_string(),
            "--timeout".to_string(),
            "5".to_string(),
        ]);
        if command == "format" {
            args.push("--stdout".to_string());
        }
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = fixture.context().run(&refs);

        assert!(output.ensure_success().is_err(), "{command} should fail");
        assert!(output.stderr_text().contains(expected), "{command}");
    }
}
