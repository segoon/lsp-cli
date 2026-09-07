use std::path::Path;
use std::time::{Duration, Instant};

use crate::harness::{E2eContext, SocketSnapshot};
use crate::lsp_exchange;
use crate::manifest::{Manifest, RealServerLifecycleCase};
use crate::repository_root;

struct LifecycleTest<'a> {
    case: RealServerLifecycleCase<'a>,
    repository: &'a Path,
}

impl<'a> LifecycleTest<'a> {
    fn new(case: RealServerLifecycleCase<'a>, repository: &'a Path) -> Self {
        Self { case, repository }
    }

    fn run(self) -> Result<(), String> {
        let label = self.case.label();
        eprintln!("E2E lifecycle {label}: started");
        let result = self
            .run_inner()
            .map_err(|error| format!("E2E lifecycle {label} failed:\n{error}"));
        eprintln!("E2E lifecycle {label}: finished");
        result
    }

    fn run_inner(&self) -> Result<(), String> {
        let started = Instant::now();
        let deadline = Duration::from_secs(self.case.deadline_seconds());
        E2eContext::run_cleaned(|context| {
            let source = self.repository.join(self.case.project());
            context.copy_project(&source)?;
            for (name, resolver) in self.case.host_programs() {
                context.stage_host_program(name, resolver, remaining(started, deadline)?)?;
            }
            let server = self.case.server_name(self.repository)?;
            if self.case.direct_run_enabled() {
                self.direct_run(context, &server, remaining(started, deadline)?)?;
            }
            self.detached(context, &source, &server, started, deadline)
                .map_err(|error| format!("{error}\n{}", context.lifecycle_state()))
        })
    }

    fn direct_run(
        &self,
        context: &E2eContext,
        server: &str,
        remaining: Duration,
    ) -> Result<(), String> {
        let args = self.server_args("run", ".", server);
        lsp_exchange::run(context, &args, context.workspace(), remaining)
            .map_err(|error| format!("direct run protocol exchange failed:\n{error}"))
    }

    fn detached(
        &self,
        context: &E2eContext,
        source: &Path,
        server: &str,
        started: Instant,
        deadline: Duration,
    ) -> Result<(), String> {
        let daemon = context.try_run_with_deadline(
            &refs(&self.daemon_args(".", server)),
            remaining(started, deadline)?,
        )?;
        daemon.ensure_success()?;
        let initial = only_socket(context)?;
        if daemon.stdout_text().trim() != initial.path.display().to_string() {
            return Err(format!(
                "daemon reported {}, but created {}",
                daemon.stdout_text().trim(),
                initial.path.display()
            ));
        }

        self.detached_query(context, ".", server, remaining(started, deadline)?)?;
        let after_first = only_socket(context)?;
        let starts = context.server_start_count();
        self.detached_query(context, ".", server, remaining(started, deadline)?)?;
        let after_second = only_socket(context)?;
        if initial != after_first
            || after_first != after_second
            || context.server_start_count() != starts
        {
            return Err("detached queries did not reuse the original daemon/server".to_string());
        }

        let stopped_pids = context.server_pids();
        let stop = context.try_run_with_deadline(
            &["stop", ".", "--lang", self.case.language(), "--lsp", server],
            remaining(started, deadline)?,
        )?;
        stop.ensure_success()?;
        expect_socket_count(context, 0)?;
        context.wait_for_processes_to_exit(
            &stopped_pids,
            remaining(started, deadline)?.min(Duration::from_secs(10)),
        )?;

        self.detached_query(context, ".", server, remaining(started, deadline)?)?;
        let restarted = only_socket(context)?;
        if restarted == initial || context.server_start_count() <= starts {
            return Err(
                "a detached query after stop did not start a fresh daemon/server".to_string(),
            );
        }

        let alternate = context.copy_project_as(source, "alternate-workspace")?;
        let alternate_text = alternate
            .to_str()
            .ok_or_else(|| "alternate workspace path is not UTF-8".to_string())?;
        self.detached_query(
            context,
            alternate_text,
            server,
            remaining(started, deadline)?,
        )?;
        expect_socket_count(context, 2)?;
        let stopped_pids = context.server_pids();
        let stop_all =
            context.try_run_with_deadline(&["stop-all"], remaining(started, deadline)?)?;
        stop_all.ensure_success()?;
        expect_socket_count(context, 0)?;
        context.wait_for_processes_to_exit(
            &stopped_pids,
            remaining(started, deadline)?.min(Duration::from_secs(10)),
        )
    }

    fn detached_query(
        &self,
        context: &E2eContext,
        workspace: &str,
        server: &str,
        deadline: Duration,
    ) -> Result<(), String> {
        let mut args = vec![
            "server-capabilities".to_string(),
            workspace.to_string(),
            "--lang".to_string(),
            self.case.language().to_string(),
            "--lsp".to_string(),
            server.to_string(),
            "--detach".to_string(),
            "--timeout".to_string(),
            self.case.lsp_timeout_seconds().to_string(),
            "--json".to_string(),
            "--debug".to_string(),
        ];
        args.push("--download".to_string());
        let output = context.try_run_with_deadline(&refs(&args), deadline)?;
        output.ensure_success()?;
        let response: serde_json::Value = output.try_json()?;
        let capabilities = response
            .get("capabilities")
            .ok_or_else(|| "server-capabilities response omitted capabilities".to_string())?;
        context.record_server_capabilities(capabilities)
    }

    fn server_args(&self, command: &str, workspace: &str, server: &str) -> Vec<String> {
        let mut args = vec![
            command.to_string(),
            workspace.to_string(),
            "--lang".to_string(),
            self.case.language().to_string(),
            "--lsp".to_string(),
            server.to_string(),
            "--debug".to_string(),
        ];
        args.push("--download".to_string());
        args
    }

    fn daemon_args(&self, workspace: &str, server: &str) -> Vec<String> {
        let mut args = self.server_args("daemon", workspace, server);
        args.extend(["--idle-timeout".to_string(), "120".to_string()]);
        args
    }
}

fn refs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

fn only_socket(context: &E2eContext) -> Result<SocketSnapshot, String> {
    let sockets = context.daemon_sockets()?;
    let [socket] = sockets.as_slice() else {
        return Err(format!("expected one daemon socket, found {sockets:#?}"));
    };
    Ok(socket.clone())
}

fn expect_socket_count(context: &E2eContext, expected: usize) -> Result<(), String> {
    let sockets = context.daemon_sockets()?;
    if sockets.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "expected {expected} daemon sockets, found {sockets:#?}"
        ))
    }
}

fn remaining(started: Instant, deadline: Duration) -> Result<Duration, String> {
    let remaining = deadline.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        Err(format!(
            "lifecycle case exceeded its overall deadline of {deadline:?}"
        ))
    } else {
        Ok(remaining)
    }
}

#[test]
#[ignore = "downloads and runs real LSP servers; executed explicitly in CI"]
fn manifest_real_server_lifecycle_cases() {
    let repository = repository_root();
    let manifest = Manifest::load_validated(repository).expect("E2E manifest should be valid");
    let selected = std::env::var("E2E_CASE").ok();
    assert!(
        selected
            .as_deref()
            .is_none_or(|label| manifest.declares_pair(label)),
        "E2E_CASE {:?} does not select a declared manifest pair",
        selected.as_deref().unwrap_or_default()
    );
    let cases = manifest
        .real_server_lifecycle_cases()
        .filter(|case| {
            selected
                .as_ref()
                .is_none_or(|expected| case.label() == *expected)
        })
        .collect::<Vec<_>>();
    let failures = cases
        .into_iter()
        .filter_map(|case| LifecycleTest::new(case, repository).run().err())
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "real-server lifecycle failures:\n{}",
        failures.join("\n\n")
    );
}
