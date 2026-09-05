use crate::fixture::E2eFixture;
use crate::manifest::CommandStrategy;
use std::collections::BTreeSet;

#[test]
fn catalog_command_paths_are_covered() {
    let fixture = E2eFixture::new().expect("E2E fixture should initialize");
    let context = fixture.context();

    let commands = context.run(&["commands"]);
    commands.assert_success();
    let actual = commands.stdout_text().lines().collect::<BTreeSet<_>>();
    assert_eq!(actual, fixture.manifest().command_names());

    for command in fixture.commands_for(CommandStrategy::Catalog) {
        let output = match command {
            "commands" => continue,
            "languages" => context.run(&[command]),
            "servers" => context.run(&[command, "--lang", "rust"]),
            "completion" => context.run(&[command, "bash"]),
            "agent-skill" => context.run(&[command]),
            other => panic!("catalog strategy has no scenario for {other:?}"),
        };
        output.assert_success();
        assert!(
            !output.stdout_text().is_empty(),
            "{command} should produce identifying output"
        );
        assert!(output.stderr_text().is_empty());
    }
}
