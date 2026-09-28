use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::dependencies::ManagedDependencies;
use crate::manifest::{Manifest, ServerProvisioningCase};
use crate::real_server_support::{CaseDeadline, RunReport, run_isolated_case, run_reported_case};
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

pub(crate) fn run_cases(
    manifest: &Manifest,
    repository: &Path,
    dependencies: &ManagedDependencies,
    include: impl Fn(&str) -> bool,
) -> RunReport {
    let mut report = RunReport::default();
    for case in manifest.server_provisioning_cases() {
        if include(case.server_id()) {
            report.record(ProvisioningTest::new(case, repository, dependencies).run());
        }
    }
    report
}
