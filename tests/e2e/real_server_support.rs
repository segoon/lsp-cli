use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::dependencies::ManagedDependencies;
use crate::harness::E2eContext;
#[cfg(test)]
use crate::results::E2eFailure;
use crate::results::{AtStage, CaseKind, E2eResult, FailureStage, record_case};

const RUNTIME_PROGRAMS: [&str; 2] = ["node", "dotnet"];

#[derive(Default)]
pub(crate) struct RunReport {
    pub(crate) planned: usize,
    pub(crate) failures: Vec<String>,
}

impl RunReport {
    pub(crate) fn record(&mut self, result: Result<(), String>) {
        self.planned += 1;
        if let Err(error) = result {
            self.failures.push(error);
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.planned += other.planned;
        self.failures.extend(other.failures);
    }
}

pub(crate) struct CaseDeadline {
    started: Instant,
    limit: Duration,
    description: &'static str,
}

impl CaseDeadline {
    pub(crate) fn new(seconds: u64, description: &'static str) -> Self {
        Self {
            started: Instant::now(),
            limit: Duration::from_secs(seconds),
            description,
        }
    }

    pub(crate) fn remaining(&self) -> Result<Duration, String> {
        let remaining = self.limit.saturating_sub(self.started.elapsed());
        if remaining.is_zero() {
            Err(format!(
                "{} exceeded its overall deadline of {:?}",
                self.description, self.limit
            ))
        } else {
            Ok(remaining)
        }
    }
}

pub(crate) fn run_isolated_case<'a>(
    dependencies: &ManagedDependencies,
    project: &Path,
    host_programs: impl IntoIterator<Item = (&'a str, &'a [String])>,
    deadline: &CaseDeadline,
    operation: impl FnOnce(&E2eContext) -> E2eResult,
) -> E2eResult {
    E2eContext::run_cleaned_real_server(dependencies, |context| {
        context
            .copy_project(project)
            .at_stage(FailureStage::Setup)?;
        let runtime_programs = RUNTIME_PROGRAMS.map(str::to_string);
        let host_programs = merged_host_programs(&runtime_programs, host_programs);
        for (name, resolver) in host_programs {
            let remaining = deadline.remaining().at_stage(FailureStage::Setup)?;
            context
                .stage_host_program(&name, &resolver, remaining)
                .at_stage(FailureStage::Setup)?;
        }
        operation(context)
    })
}

fn merged_host_programs<'a>(
    runtime_programs: &[String],
    explicit_programs: impl IntoIterator<Item = (&'a str, &'a [String])>,
) -> BTreeMap<String, Vec<String>> {
    let mut programs = runtime_programs
        .iter()
        .map(|name| (name.clone(), vec!["which".to_string(), name.to_string()]))
        .collect::<BTreeMap<_, _>>();
    programs.extend(
        explicit_programs
            .into_iter()
            .map(|(name, resolver)| (name.to_string(), resolver.to_vec())),
    );
    programs
}

pub(crate) fn run_reported_case(
    kind: CaseKind,
    label: &str,
    operation: impl FnOnce() -> E2eResult,
) -> Result<(), String> {
    let diagnostic_kind = kind.diagnostic_label();
    eprintln!("E2E {diagnostic_kind} {label}: started");
    let result = operation();
    let record = record_case(kind, label, &result);
    eprintln!("E2E {diagnostic_kind} {label}: finished");
    match (result, record) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(format!(
            "E2E {diagnostic_kind} {label} failed:\n{}",
            error.render()
        )),
        (Ok(()), Err(error)) => Err(format!(
            "E2E {diagnostic_kind} {label} failed:\nE2E failure stage: setup\nfailed to record E2E result: {error}"
        )),
        (Err(error), Err(record_error)) => Err(format!(
            "E2E {diagnostic_kind} {label} failed:\n{}\nfailed to record E2E result: {record_error}",
            error.render()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reported_failure_names_its_case() {
        let error = run_reported_case(CaseKind::Lifecycle, "go/gopls", || {
            Err(E2eFailure::new(FailureStage::Lifecycle, "failed"))
        })
        .expect_err("synthetic case should fail");

        assert_eq!(
            error,
            "E2E lifecycle go/gopls failed:\nE2E failure stage: lifecycle\nfailed"
        );
    }

    #[test]
    fn exhausted_deadline_names_its_scope() {
        let error = CaseDeadline::new(0, "server provisioning")
            .remaining()
            .expect_err("zero deadline should be exhausted");

        assert_eq!(
            error,
            "server provisioning exceeded its overall deadline of 0ns"
        );
    }

    #[test]
    fn explicit_host_program_overrides_deduplicated_runtime_default() {
        let runtime_programs = vec!["node".to_string(), "node".to_string()];
        let explicit_resolver = vec!["custom-node".to_string()];
        let programs =
            merged_host_programs(&runtime_programs, [("node", explicit_resolver.as_slice())]);

        assert_eq!(programs.len(), 1);
        assert_eq!(programs["node"], explicit_resolver);
    }
}
