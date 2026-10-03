use std::os::unix::process::CommandExt as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt as _;

use super::ChildReaper;
use crate::harness::E2eContext;
use crate::results::{E2eFailure, E2eResult, FailureStage};

#[path = "fixture.rs"]
mod fixture;
use fixture::Fixture;

pub(crate) fn run() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [mode, role, socket, workspace] if mode == "--fixture" => {
            fixture::serve(role, Path::new(socket), Path::new(workspace));
        }
        [mode, scenario] if mode == "--scenario" => {
            ChildReaper::enable().expect("enable scenario subreaper");
            match scenario.as_str() {
                "success" | "failure" | "panic" | "zombies" | "stop-error" => {
                    case_cleanup(scenario)
                }
                "deadline" | "enumeration" | "invalid-pid" | "empty-list" => {
                    incomplete_cleanup(scenario)
                }
                _ => panic!("unknown reaper regression scenario"),
            }
        }
        [] => scenarios(),
        _ => panic!("unknown reaper regression arguments"),
    }
}

fn scenarios() {
    ChildReaper::enable().expect("enable regression backstop");
    let scenarios = [
        "success",
        "failure",
        "panic",
        "zombies",
        "stop-error",
        "deadline",
        "enumeration",
        "invalid-pid",
        "empty-list",
    ];
    for scenario in scenarios {
        run_scenarios(&[scenario]);
    }
    // Concurrent independent subreapers must never signal or reap one another's descendants.
    run_scenarios(&["success", "success"]);
}

fn run_scenarios(scenarios: &[&str]) {
    let mut backstop = ChildReaper::begin_case().expect("regression backstop");
    let executable = std::env::current_exe().expect("regression executable");
    let mut children = scenarios
        .iter()
        .map(|scenario| {
            Command::new(&executable)
                .args(["--scenario", scenario])
                // A regression in PID validation must not signal the parent test command's group.
                .process_group(0)
                .stdin(Stdio::null())
                .spawn()
                .expect("start regression scenario")
        })
        .collect::<Vec<_>>();
    let mut failures = Vec::new();
    for (scenario, child) in scenarios.iter().zip(&mut children) {
        match child
            .wait_timeout(Duration::from_secs(30))
            .expect("wait for scenario")
        {
            Some(status) if status.success() => {}
            Some(status) => failures.push(format!("{scenario}: {status}")),
            None => {
                child.kill().expect("kill timed-out scenario");
                child.wait().expect("reap timed-out scenario");
                failures.push(format!("{scenario}: timed out"));
            }
        }
    }
    backstop
        .finish()
        .expect("clean up scenario descendants even after assertion failure");
    assert!(
        failures.is_empty(),
        "reaper regression failures: {failures:?}"
    );
}

fn supervised_context(reaper: ChildReaper) -> E2eContext {
    E2eContext::new()
        .expect("regression context")
        .with_reaper(reaper)
}

fn case_cleanup(scenario: &str) {
    if scenario == "panic" {
        // This child deliberately panics and verifies its payload after cleanup. Avoid reporting
        // the expected panic as an apparent failure in the parent test command's output.
        std::panic::set_hook(Box::new(|_| {}));
    }
    let context = supervised_context(ChildReaper::begin_case().expect("case reaper"));
    let roots = context.isolated_roots();
    let mut fixture = None;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        E2eContext::run_cleaned_context(Ok(context), |context| -> E2eResult {
            let mut tree = Fixture::start(context.workspace(), true);
            tree.orphan();
            tree.instruct("branch", b'w');
            tree.instruct("leaf", b'w');
            if scenario == "zombies" {
                tree.instruct("branch", b'x');
                context
                    .wait_for_processes_to_exit(&[tree.pid("branch")], Duration::from_secs(5))
                    .expect("adopted zombie parent should be reaped without killing its child");
                tree.instruct("leaf", b'w');
                tree.instruct("leaf", b'x');
                context
                    .wait_for_processes_to_exit(&[tree.pid("leaf")], Duration::from_secs(5))
                    .expect("adopted zombie should be reaped during lifecycle checks");
            }
            fixture = Some(tree);
            if scenario == "stop-error" {
                // A file in place of the daemon directory makes the actual stop-all command fail.
                // Finalization must still kill descendants and preserve the shutdown diagnostic.
                let [_, runtime] = context.isolated_roots();
                std::fs::write(runtime.join("lsp-cli"), "not a directory").expect("break stop-all");
            }
            match scenario {
                "failure" | "stop-error" => Err(E2eFailure::new(
                    FailureStage::Lifecycle,
                    "original case failure",
                )),
                "panic" => panic!("original case panic"),
                _ => Ok(()),
            }
        })
    }));
    fixture.expect("case fixture").assert_gone();
    assert!(
        roots.iter().all(|root| !root.exists()),
        "case directories should be removed"
    );
    match (scenario, outcome) {
        ("panic", Err(payload)) => {
            assert_eq!(payload.downcast_ref::<&str>(), Some(&"original case panic"))
        }
        ("failure", Ok(Err(error))) => assert!(error.render().contains("original case failure")),
        ("stop-error", Ok(Err(error))) => {
            let diagnostic = error.render();
            for expected in [
                "original case failure",
                "additional failure stages: cleanup",
                "daemon cleanup exited unsuccessfully",
            ] {
                assert!(
                    diagnostic.contains(expected),
                    "missing {expected:?}: {diagnostic}"
                );
            }
        }
        ("success" | "zombies", Ok(Ok(()))) => {}
        _ => panic!("case outcome was not preserved"),
    }
    let mut next =
        ChildReaper::begin_case().expect("successful cleanup should allow the next case");
    next.finish().expect("no children remain");
}

fn incomplete_cleanup(scenario: &str) {
    let mut reaper = ChildReaper::begin_case().expect("case reaper");
    let scratch = tempfile::tempdir().expect("enumeration fixture directory");
    match scenario {
        "deadline" => reaper.timeout = Duration::ZERO,
        "enumeration" => reaper.children_file = scratch.path().join("missing"),
        "invalid-pid" => {
            reaper.children_file = scratch.path().join("children");
            std::fs::write(&reaper.children_file, "0").expect("invalid process list");
        }
        "empty-list" => {
            reaper.timeout = Duration::from_millis(50);
            reaper.children_file = scratch.path().join("children");
            std::fs::write(&reaper.children_file, "").expect("incomplete process list");
        }
        _ => panic!("unknown incomplete cleanup scenario"),
    }
    let context = supervised_context(reaper);
    let roots = context.isolated_roots();
    let mut fixture = None;
    let result = E2eContext::run_cleaned_context(Ok(context), |context| -> E2eResult {
        fixture = Some(Fixture::start(context.workspace(), false));
        Err(E2eFailure::new(
            FailureStage::Lifecycle,
            "original case failure",
        ))
    });
    let error = result.expect_err("incomplete cleanup should fail").render();
    for expected in [
        "original case failure",
        "additional failure stages: cleanup",
        "sandbox root: not removed",
        "runtime root: not removed",
    ] {
        assert!(
            error.contains(expected),
            "missing diagnostic {expected:?}: {error}"
        );
    }
    assert!(
        roots.iter().all(|root| root.exists()),
        "unsafe directory removal"
    );
    let mut fixture = fixture.expect("retained fixture");
    fixture.instruct("leaf", b'w');
    fixture.stop_direct_child();
    assert!(
        ChildReaper::begin_case().is_err(),
        "failed cleanup must prevent later cases even after children exit"
    );
    for root in roots {
        std::fs::remove_dir_all(root).expect("remove retained fixture directory");
    }
}
