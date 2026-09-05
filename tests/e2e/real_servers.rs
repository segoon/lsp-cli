use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::harness::E2eContext;
use crate::manifest::{Manifest, ProvisionMethod, QueryKind, RealServerCase};

struct RealServerTest<'a> {
    case: RealServerCase<'a>,
    repository: &'a Path,
}

#[derive(Deserialize)]
struct ListSymbolsOutput {
    detected: BTreeSet<String>,
    server: ServerOutput,
    matches: Vec<SymbolOutput>,
}

#[derive(Deserialize)]
struct ServerOutput {
    command: Vec<String>,
}

#[derive(Deserialize)]
struct SymbolOutput {
    name: String,
}

impl<'a> RealServerTest<'a> {
    fn new(case: RealServerCase<'a>, repository: &'a Path) -> Self {
        Self { case, repository }
    }

    fn run(self) -> Result<(), String> {
        self.run_inner()
            .map_err(|error| format!("E2E case {} failed:\n{error}", self.case.label()))
    }

    fn run_inner(&self) -> Result<(), String> {
        let started = Instant::now();
        let deadline = Duration::from_secs(self.case.deadline_seconds());
        let context = E2eContext::new()
            .map_err(|error| format!("failed to create an isolated E2E context: {error}"))?;
        context.copy_project(&self.repository.join(self.case.project()))?;

        for (name, resolver) in self.case.host_programs() {
            context.stage_host_program(name, resolver, remaining(started, deadline)?)?;
        }

        let server_name = self.case.server_name(self.repository)?;
        let args = self.command_args(&server_name);
        let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = context.try_run_with_deadline(&arg_refs, remaining(started, deadline)?)?;
        output.ensure_success()?;

        match self.case.query_kind() {
            QueryKind::ListSymbols => {
                let response: ListSymbolsOutput = output.try_json()?;
                self.validate_list_symbols(&context, &response)
            }
        }
    }

    fn command_args(&self, server_name: &str) -> Vec<String> {
        let mut args = match self.case.query_kind() {
            QueryKind::ListSymbols => vec!["list-symbols".to_string(), ".".to_string()],
        };
        args.extend(["--lsp".to_string(), server_name.to_string()]);
        match self.case.provision_method() {
            ProvisionMethod::Download => args.push("--download".to_string()),
        }
        args.extend([
            "--no-detach".to_string(),
            "--json".to_string(),
            "--timeout".to_string(),
            self.case.lsp_timeout_seconds().to_string(),
        ]);
        args
    }

    fn validate_list_symbols(
        &self,
        context: &E2eContext,
        response: &ListSymbolsOutput,
    ) -> Result<(), String> {
        if !response.detected.contains(self.case.language()) {
            return Err(format!(
                "expected detected languages {:?} to contain {:?}",
                response.detected,
                self.case.language()
            ));
        }

        let Some(program) = response.server.command.first() else {
            return Err("selected server reported an empty command".to_string());
        };
        if !Path::new(program).starts_with(context.home()) {
            return Err(format!(
                "downloaded server program {} is outside isolated home {}",
                program,
                context.home().display()
            ));
        }

        let actual = response
            .matches
            .iter()
            .map(|matched| matched.name.as_str())
            .collect::<BTreeSet<_>>();
        let missing = self
            .case
            .expected_names()
            .iter()
            .filter(|expected| !actual.contains(expected.as_str()))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "missing expected symbols {missing:?}; returned symbols: {actual:?}"
            ))
        }
    }
}

fn remaining(started: Instant, deadline: Duration) -> Result<Duration, String> {
    let remaining = deadline.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        Err(format!(
            "case exceeded its overall deadline of {deadline:?}"
        ))
    } else {
        Ok(remaining)
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
#[ignore = "downloads and runs real LSP servers; executed explicitly in CI"]
fn manifest_real_server_smoke_cases() {
    let repository = repository_root();
    let manifest = Manifest::load_validated(&repository).expect("E2E manifest should be valid");
    let failures = manifest
        .real_server_smoke_cases()
        .filter_map(|case| RealServerTest::new(case, &repository).run().err())
        .collect::<Vec<_>>();

    assert!(
        failures.is_empty(),
        "real-server E2E failures:\n{}",
        failures.join("\n\n")
    );
}
