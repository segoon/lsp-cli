use super::{lsp_workspace_query, parse};
use crate::cli::{Command, NavigationArgs};
use crate::navigation::NavigationTarget;

const COMMANDS: [&str; 7] = [
    "definition",
    "declaration",
    "type-definition",
    "implementation",
    "references",
    "callers",
    "callees",
];

fn navigation(args: &[&str]) -> NavigationArgs {
    match parse(args).expect("navigation parses") {
        Command::Definition(args)
        | Command::Declaration(args)
        | Command::TypeDefinition(args)
        | Command::Implementation(args)
        | Command::References(args)
        | Command::Callers(args)
        | Command::Callees(args) => args,
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn navigation_commands_share_three_target_forms_and_defaults() {
    for command in COMMANDS {
        for directory in [None, Some("workspace")] {
            let mut invocation = vec![command, "Target"];
            if let Some(directory) = directory {
                invocation.push(directory);
            }
            let args = navigation(&invocation);
            assert_eq!(args.target, NavigationTarget::Name("Target".into()));
            assert_eq!(args.query, lsp_workspace_query(directory.unwrap_or(".")));
            assert!(!args.full);
        }
        for name in [None, Some("Target")] {
            let mut invocation = vec![command, "--at", "src/a:b.rs:2:3"];
            if let Some(name) = name {
                invocation.push(name);
            }
            let args = navigation(&invocation);
            assert_eq!(
                args.query.query.directory,
                std::path::Path::new("src/a:b.rs")
            );
            assert_eq!(
                args.target,
                NavigationTarget::Position {
                    location: "src/a:b.rs:2:3".parse().expect("location"),
                    expected_name: name.map(str::to_string),
                }
            );
        }
    }
    assert_eq!(
        navigation(&["ref", "Target"]).target,
        NavigationTarget::Name("Target".into())
    );
}

#[test]
fn navigation_commands_share_options_and_reject_invalid_combinations() {
    for command in COMMANDS {
        let args = navigation(&[
            command,
            "--at",
            "a.rs:1:1",
            "Target",
            "--lang",
            "rust",
            "--lsp",
            "rust-analyzer",
            "--download",
            "--detach",
            "--json",
            "--timeout",
            "500ms",
            "--limit",
            "3",
            "-l",
        ]);
        assert_eq!(args.query.query.language(), Some("rust"));
        assert_eq!(args.query.query.server(), Some("rust-analyzer"));
        assert_eq!(args.query.query.limit, 3);
        assert_eq!(
            args.query.query.timeout,
            std::time::Duration::from_millis(500)
        );
        assert!(
            args.query.download
                && args.query.detach
                && args.query.query.json
                && args.query.files_with_matches
        );
        parse(&[command]).expect_err("invalid navigation arguments");
        parse(&[command, "--at", "a.rs:1:1", "Target", "workspace"])
            .expect_err("invalid navigation arguments");
        parse(&[command, ""]).expect_err("invalid navigation arguments");
        let supports_full = !matches!(command, "references" | "callers" | "callees");
        assert_eq!(parse(&[command, "Target", "--full"]).is_ok(), supports_full);
        parse(&[command, "Target", "--full", "-l"]).expect_err("invalid navigation arguments");
    }
}
