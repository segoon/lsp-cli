use std::path::Path;
use std::time::Duration;

use crate::dependencies::ManagedDependencies;
use crate::harness::{E2eContext, SocketSnapshot};
use crate::lsp_exchange;
use crate::manifest::{Manifest, RealServerLifecycleCase};
use crate::real_server_support::{CaseDeadline, RunReport, run_isolated_case, run_reported_case};
use crate::results::{AtStage, CaseKind, E2eResult, FailureStage};

struct LifecycleTest<'a> {
    case: RealServerLifecycleCase<'a>,
    repository: &'a Path,
    dependencies: &'a ManagedDependencies,
}

impl<'a> LifecycleTest<'a> {
    fn new(
        case: RealServerLifecycleCase<'a>,
        repository: &'a Path,
        dependencies: &'a ManagedDependencies,
    ) -> Self {
        Self {
            case,
            repository,
            dependencies,
        }
    }

    fn run(self) -> Result<(), String> {
        let label = self.case.label();
        run_reported_case(CaseKind::Lifecycle, &label, || self.run_inner())
    }

    fn run_inner(&self) -> E2eResult {
        let deadline = CaseDeadline::new(self.case.deadline_seconds(), "lifecycle case");
        let source = self.repository.join(self.case.project());
        run_isolated_case(
            self.dependencies,
            &source,
            self.case.host_programs(),
            &deadline,
            |context| {
                let server = self
                    .case
                    .server_name(self.repository)
                    .at_stage(FailureStage::Setup)?;
                if self.case.direct_run_enabled() {
                    let remaining = deadline.remaining().at_stage(FailureStage::Lifecycle)?;
                    self.direct_run(context, &server, remaining)
                        .at_stage(FailureStage::Lifecycle)?;
                }
                let detached = self
                    .detached(context, &source, &server, &deadline)
                    .map_err(|error| format!("{error}\n{}", context.lifecycle_state()));
                detached.at_stage(FailureStage::Lifecycle)
            },
        )
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
        deadline: &CaseDeadline,
    ) -> Result<(), String> {
        let daemon = context
            .try_run_with_deadline(&refs(&self.daemon_args(".", server)), deadline.remaining()?)?;
        daemon.ensure_success()?;
        let initial = only_socket(context)?;
        if daemon.stdout_text().trim() != initial.path.display().to_string() {
            return Err(format!(
                "daemon reported {}, but created {}",
                daemon.stdout_text().trim(),
                initial.path.display()
            ));
        }

        self.detached_query(context, ".", server, deadline.remaining()?)?;
        let after_first = only_socket(context)?;
        let starts = context.server_start_count();
        self.detached_query(context, ".", server, deadline.remaining()?)?;
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
            deadline.remaining()?,
        )?;
        stop.ensure_success()?;
        expect_socket_count(context, 0)?;
        context.wait_for_processes_to_exit(
            &stopped_pids,
            deadline.remaining()?.min(Duration::from_secs(10)),
        )?;

        self.detached_query(context, ".", server, deadline.remaining()?)?;
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
        self.detached_query(context, alternate_text, server, deadline.remaining()?)?;
        expect_socket_count(context, 2)?;
        let stopped_pids = context.server_pids();
        let stop_all = context.try_run_with_deadline(&["stop-all"], deadline.remaining()?)?;
        stop_all.ensure_success()?;
        expect_socket_count(context, 0)?;
        context.wait_for_processes_to_exit(
            &stopped_pids,
            deadline.remaining()?.min(Duration::from_secs(10)),
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

pub(crate) fn run_cases(
    manifest: &Manifest,
    repository: &Path,
    dependencies: &ManagedDependencies,
    include: impl Fn(&str, &str, bool) -> bool,
) -> RunReport {
    let mut report = RunReport::default();
    for case in manifest.real_server_lifecycle_cases() {
        if include(&case.label(), case.server_id(), case.is_smoke()) {
            report.record(LifecycleTest::new(case, repository, dependencies).run());
        }
    }
    report
}
