use std::path::Path;
use std::time::{Duration, Instant};

use crate::harness::E2eContext;
#[cfg(test)]
use crate::results::E2eFailure;
use crate::results::{AtStage, CaseKind, E2eResult, FailureStage, record_case};

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
    project: &Path,
    host_programs: impl IntoIterator<Item = (&'a str, &'a [String])>,
    deadline: &CaseDeadline,
    operation: impl FnOnce(&E2eContext) -> E2eResult,
) -> E2eResult {
    E2eContext::run_cleaned_real_server(|context| {
        context
            .copy_project(project)
            .at_stage(FailureStage::Setup)?;
        for (name, resolver) in host_programs {
            let remaining = deadline.remaining().at_stage(FailureStage::Setup)?;
            context
                .stage_host_program(name, resolver, remaining)
                .at_stage(FailureStage::Setup)?;
        }
        operation(context)
    })
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
}
