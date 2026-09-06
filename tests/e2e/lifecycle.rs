use crate::local_fixture::LocalFixture;
use crate::lsp_exchange;
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
    stop.assert_stdout_contains("stopped");

    // Use a distinct socket so stop-all coverage does not also depend on same-socket restart timing.
    let stop_all_workspace = context
        .copy_project_as(context.workspace(), "stop-all-workspace")
        .expect("stop-all workspace should initialize");
    let stop_all_workspace = stop_all_workspace.display().to_string();
    context
        .run(&[
            "daemon",
            &stop_all_workspace,
            "--lsp",
            server,
            "--no-download",
            "--idle-timeout",
            "10",
        ])
        .assert_success();
    let stop_all = context.run(&["stop-all"]);
    stop_all.assert_success();
    stop_all.assert_stdout_contains("stopped");

    let run = context.run_with_env(
        &["run", ".", "--lsp", server, "--no-download"],
        &[("LSP_CLI_E2E_RUN_MARKER", "1")],
    );
    run.assert_success();
    assert_eq!(run.stdout_text(), "fake LSP server replaced lsp-cli\n");
}

#[test]
fn run_forwards_a_complete_lsp_exchange() {
    let fixture = LocalFixture::new().expect("local fixture should initialize");
    let args = [
        "run".to_string(),
        ".".to_string(),
        "--lsp".to_string(),
        fixture.server_name().to_string(),
        "--no-download".to_string(),
        "--debug".to_string(),
    ];

    lsp_exchange::run(
        fixture.context(),
        &args,
        fixture.context().workspace(),
        std::time::Duration::from_secs(10),
    )
    .expect("run should forward a complete LSP exchange");
}
