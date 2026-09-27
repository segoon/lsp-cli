use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::dependencies::ManagedDependencies;
use crate::manifest::{Manifest, ServerProvisioningCase};
use crate::real_server_support::{CaseDeadline, run_isolated_case, run_reported_case};
use crate::repository_root;
use crate::results::{AtStage, CaseKind, E2eResult, FailureStage};

struct ProvisioningTest<'a> {
    case: ServerProvisioningCase<'a>,
    repository: &'a Path,
    dependencies: &'a ManagedDependencies,
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
    fn new(
        case: ServerProvisioningCase<'a>,
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
        let id = self.case.server_id();
        run_reported_case(CaseKind::Provisioning, id, || self.run_inner())
    }

    fn run_inner(&self) -> E2eResult {
        let deadline = CaseDeadline::new(self.case.deadline_seconds(), "server provisioning");
        run_isolated_case(
            self.dependencies,
            &self.repository.join(self.case.project()),
            self.case.host_programs(),
            &deadline,
            |context| {
                let server_name = self
                    .case
                    .server_name(self.repository)
                    .at_stage(FailureStage::Setup)?;
                let remaining = deadline.remaining().at_stage(FailureStage::Provisioning)?;
                let output = context
                    .try_run_with_deadline(
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
                        remaining,
                    )
                    .at_stage(FailureStage::Provisioning)?;
                output
                    .ensure_success()
                    .at_stage(FailureStage::Provisioning)?;
                let response: DetectOutput =
                    output.try_json().at_stage(FailureStage::Provisioning)?;
                let [server] = response.servers.as_slice() else {
                    return Err(crate::results::E2eFailure::new(
                        FailureStage::Provisioning,
                        format!(
                            "detect returned {} servers instead of exactly one",
                            response.servers.len()
                        ),
                    ));
                };
                if server.server != server_name
                    || !server
                        .languages
                        .iter()
                        .any(|item| item == self.case.language())
                {
                    return Err(crate::results::E2eFailure::new(
                        FailureStage::Provisioning,
                        format!(
                            "detect returned server {:?} for languages {:?}, expected {:?} for {:?}",
                            server.server,
                            server.languages,
                            server_name,
                            self.case.language()
                        ),
                    ));
                }
                let Some(program) = server.command.first() else {
                    return Err(crate::results::E2eFailure::new(
                        FailureStage::Provisioning,
                        "downloaded server reported an empty command",
                    ));
                };
                validate_program(program, context.home()).at_stage(FailureStage::Provisioning)
            },
        )
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

#[test]
#[ignore = "downloads real LSP servers; executed explicitly by the provisioning target"]
fn manifest_server_provisioning_cases() {
    let repository = repository_root();
    let dependencies = ManagedDependencies::prepare()
        .expect("server-provisioning E2E dependencies should be available");
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
        .filter_map(|case| {
            ProvisioningTest::new(case, repository, &dependencies)
                .run()
                .err()
        })
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "server provisioning failures:\n{}",
        failures.join("\n\n")
    );
}
