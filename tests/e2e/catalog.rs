use crate::harness::E2eContext;

#[test]
fn commands_lists_every_canonical_subcommand() {
    let output = E2eContext::new()
        .expect("E2E context should initialize")
        .run(&["commands"]);

    output.assert_success();
    assert!(output.stderr_text().is_empty());
    assert_eq!(
        output.stdout_text().trim_end(),
        concat!(
            "commands\n",
            "daemon\n",
            "stop\n",
            "stop-all\n",
            "languages\n",
            "servers\n",
            "server-capabilities\n",
            "detect\n",
            "diagnostics\n",
            "format\n",
            "grep\n",
            "list-symbols\n",
            "list-functions\n",
            "list-files\n",
            "references\n",
            "callers\n",
            "callees\n",
            "definition\n",
            "declaration\n",
            "build-index\n",
            "update\n",
            "completion\n",
            "agent-skill\n",
            "run"
        )
    );
}
