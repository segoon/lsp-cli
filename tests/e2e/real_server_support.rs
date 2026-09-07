use std::path::Path;
use std::time::{Duration, Instant};

use crate::harness::E2eContext;

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
    operation: impl FnOnce(&E2eContext) -> Result<(), String>,
) -> Result<(), String> {
    E2eContext::run_cleaned(|context| {
        context.copy_project(project)?;
        for (name, resolver) in host_programs {
            context.stage_host_program(name, resolver, deadline.remaining()?)?;
        }
        operation(context)
    })
}

pub(crate) fn run_reported_case(
    kind: &str,
    label: &str,
    operation: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    eprintln!("E2E {kind} {label}: started");
    let result = operation().map_err(|error| format!("E2E {kind} {label} failed:\n{error}"));
    eprintln!("E2E {kind} {label}: finished");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reported_failure_names_its_case() {
        let error = run_reported_case("lifecycle", "go/gopls", || Err("failed".to_string()))
            .expect_err("synthetic case should fail");

        assert_eq!(error, "E2E lifecycle go/gopls failed:\nfailed");
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
