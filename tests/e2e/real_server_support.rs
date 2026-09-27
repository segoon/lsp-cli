use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::dependencies::ManagedDependencies;
use crate::harness::E2eContext;
#[cfg(test)]
use crate::results::E2eFailure;
use crate::results::{AtStage, CaseKind, E2eResult, FailureStage, record_case};

const RUNTIME_PROGRAMS_ENV: &str = "E2E_RUNTIME_PROGRAMS";

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
        let runtime_programs = runtime_programs().at_stage(FailureStage::Setup)?;
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

fn runtime_programs() -> Result<Vec<String>, String> {
    let value = match std::env::var(RUNTIME_PROGRAMS_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(Vec::new()),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!("{RUNTIME_PROGRAMS_ENV} must contain valid UTF-8"));
        }
    };
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            validate_runtime_program_name(name)?;
            Ok(name.to_string())
        })
        .collect()
}

fn validate_runtime_program_name(name: &str) -> Result<(), String> {
    let path = Path::new(name);
    let is_bare_name = path.file_name().is_some_and(|file_name| file_name == name)
        && !name.contains(['/', '\\'])
        && name != "."
        && name != "..";
    if is_bare_name {
        Ok(())
    } else {
        Err(format!(
            "{RUNTIME_PROGRAMS_ENV} entry {name:?} must be a bare executable name"
        ))
    }
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

    #[test]
    fn runtime_program_names_reject_paths_and_traversal() {
        for name in ["../node", "tools/node", r"tools\node", ".", ".."] {
            let error = validate_runtime_program_name(name)
                .expect_err("runtime program paths should be rejected");
            assert!(error.contains("must be a bare executable name"));
        }
        validate_runtime_program_name("node").expect("a bare program name should be accepted");
    }
}
