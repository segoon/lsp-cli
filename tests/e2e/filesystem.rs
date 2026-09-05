use serde_json::Value;

use crate::local_fixture::LocalFixture;
use crate::manifest::{CommandStrategy, Manifest};

#[test]
fn filesystem_command_paths_are_covered() {
    let manifest = Manifest::load_repository().expect("E2E manifest should be valid");
    let fixture = LocalFixture::new().expect("local fixture should initialize");

    for command in manifest.commands_for(CommandStrategy::Filesystem) {
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
