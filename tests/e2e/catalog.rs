use crate::harness::E2eContext;

#[test]
fn commands_lists_every_canonical_subcommand() {
    let output = E2eContext::new()
        .expect("E2E context should initialize")
        .run(&["commands"])
        .expect("lsp-cli commands should run");

    assert!(
        output.status.success(),
        "lsp-cli commands failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("lsp-cli commands output should be UTF-8")
            .trim_end(),
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
