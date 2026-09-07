use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::harness::E2eContext;
use crate::manifest::{Manifest, ServerProvisioningCase};
use crate::repository_root;

struct ProvisioningTest<'a> {
    case: ServerProvisioningCase<'a>,
    repository: &'a Path,
}

#[derive(Deserialize)]
struct DetectOutput {
    servers: Vec<DetectedServer>,
}

#[derive(Deserialize)]
struct DetectedServer {
    languages: Vec<String>,
    server: String,
    command: Vec<String>,
}

impl<'a> ProvisioningTest<'a> {
    fn new(case: ServerProvisioningCase<'a>, repository: &'a Path) -> Self {
        Self { case, repository }
    }

    fn run(self) -> Result<(), String> {
        let id = self.case.server_id();
        eprintln!("E2E provisioning {id}: started");
        let result = self
            .run_inner()
            .map_err(|error| format!("E2E provisioning {id} failed:\n{error}"));
        eprintln!("E2E provisioning {id}: finished");
        result
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
        let output = context.try_run_with_deadline(
            &[
                "detect",
                ".",
                "--lang",
                self.case.language(),
                "--lsp",
                &server_name,
                "--download",
                "--json",
                "--debug",
            ],
            remaining(started, deadline)?,
        )?;
        output.ensure_success()?;
        let response: DetectOutput = output.try_json()?;
        let [server] = response.servers.as_slice() else {
            return Err(format!(
                "detect returned {} servers instead of exactly one",
                response.servers.len()
            ));
        };
        if server.server != server_name
            || !server
                .languages
                .iter()
                .any(|item| item == self.case.language())
        {
            return Err(format!(
                "detect returned server {:?} for languages {:?}, expected {:?} for {:?}",
                server.server,
                server.languages,
                server_name,
                self.case.language()
            ));
        }
        let Some(program) = server.command.first() else {
            return Err("downloaded server reported an empty command".to_string());
        };
        validate_program(program, context.home())
    }
}

fn validate_program(program: &str, home: &Path) -> Result<(), String> {
    let program = PathBuf::from(program);
    if !program.is_absolute() || !program.starts_with(home) {
        return Err(format!(
            "downloaded server program {} is outside isolated home {}",
            program.display(),
            home.display()
        ));
    }
    if !program.is_file() {
        return Err(format!(
            "downloaded server program {} is not a file",
            program.display()
        ));
    }
    Ok(())
}

fn remaining(started: Instant, deadline: Duration) -> Result<Duration, String> {
    let remaining = deadline.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        Err(format!(
            "server provisioning exceeded its overall deadline of {deadline:?}"
        ))
    } else {
        Ok(remaining)
    }
}

#[test]
#[ignore = "downloads real LSP servers; executed explicitly by the provisioning target"]
fn manifest_server_provisioning_cases() {
    let repository = repository_root();
    let manifest = Manifest::load_validated(repository).expect("E2E manifest should be valid");
    let selected = std::env::var("E2E_SERVER").ok();
    assert!(
        selected
            .as_deref()
            .is_none_or(|id| manifest.declares_server(id)),
        "E2E_SERVER {:?} does not select a server in the provisioning inventory",
        selected.as_deref().unwrap_or_default()
    );
    assert!(
        selected
            .as_deref()
            .is_none_or(|id| manifest.server_is_downloadable(id)),
        "E2E_SERVER {:?} selects an explicitly excluded provisioning server",
        selected.as_deref().unwrap_or_default()
    );
    let failures = manifest
        .server_provisioning_cases()
        .filter(|case| {
            selected
                .as_ref()
                .is_none_or(|expected| case.server_id() == expected)
        })
        .filter_map(|case| ProvisioningTest::new(case, repository).run().err())
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "server provisioning failures:\n{}",
        failures.join("\n\n")
    );
}
