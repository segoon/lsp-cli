use crate::local_fixture::LocalFixture;
use crate::manifest::CommandStrategy;
use std::collections::BTreeSet;

#[test]
fn lifecycle_command_paths_are_covered() {
    let fixture = LocalFixture::new().expect("local fixture should initialize");
    assert_eq!(
        fixture
            .commands_for(CommandStrategy::Lifecycle)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["daemon", "run", "stop", "stop-all"])
    );
    let context = fixture.context();
    let server = fixture.server_name();

    let daemon = context.run(&[
        "daemon",
        ".",
        "--lsp",
        server,
        "--no-download",
        "--idle-timeout",
        "10",
    ]);
    daemon.assert_success();
    assert!(daemon.stdout_text().trim_end().ends_with(".sock"));

    let stop = context.run(&["stop", ".", "--lsp", server]);
    stop.assert_success();
    assert!(stop.stdout_text().contains("stopped"));

    context
        .run(&[
            "daemon",
            ".",
            "--lsp",
            server,
            "--no-download",
            "--idle-timeout",
            "10",
        ])
        .assert_success();
    let stop_all = context.run(&["stop-all"]);
    stop_all.assert_success();
    assert!(stop_all.stdout_text().contains("stopped"));

    let run = context.run_with_env(
        &["run", ".", "--lsp", server, "--no-download"],
        &[("LSP_CLI_E2E_RUN_MARKER", "1")],
    );
    run.assert_success();
    assert_eq!(run.stdout_text(), "fake LSP server replaced lsp-cli\n");
}
