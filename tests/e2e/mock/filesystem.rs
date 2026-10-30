use serde_json::Value;

use super::local_fixture::LocalFixture;
use crate::manifest::CommandStrategy;

pub fn filesystem_command_paths_are_covered() {
    let fixture = LocalFixture::new().expect("local fixture should initialize");

    for command in fixture.commands_for(CommandStrategy::Filesystem) {
        if command == "cat" {
            let output = fixture.context().run(&[command, "main.fake"]);
            output.assert_success();
            assert_eq!(
                output.stdout_text(),
                include_str!("../fixtures/project/main.fake")
            );
            assert!(output.stderr_text().is_empty());
            continue;
        }
        let output =
            fixture
                .context()
                .run(&[command, ".", "--lsp", fixture.server_name(), "--json"]);
        output.assert_success();
        let value: Value = output.json();
        match command {
            "detect" => assert_eq!(
                value.pointer("/servers/0/server").and_then(Value::as_str),
                Some(fixture.server_name())
            ),
            "list-files" => assert_eq!(
                value.pointer("/files/0").and_then(Value::as_str),
                Some("./main.fake")
            ),
            other => panic!("filesystem strategy has no scenario for {other:?}"),
        }
    }
}
