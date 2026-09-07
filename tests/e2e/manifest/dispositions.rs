use std::collections::BTreeSet;

use super::{ExceptionOutcome, PairCase, SmokeDisposition};

impl SmokeDisposition {
    pub(super) fn validate(&self, pair: &PairCase) -> Result<(), String> {
        let label = format!("{}/{}", pair.language, pair.server);
        match self {
            Self::Excluded { reason } => {
                require_text(reason, &format!("E2E exclusion for {label}"))
            }
            Self::Capabilities {
                lsp_timeout_seconds,
                deadline_seconds,
            } => validate_deadlines(&label, *lsp_timeout_seconds, *deadline_seconds),
            Self::Queries {
                exceptions,
                lsp_timeout_seconds,
                deadline_seconds,
            } => validate_queries(&label, exceptions, *lsp_timeout_seconds, *deadline_seconds),
        }
    }
}

fn validate_queries(
    label: &str,
    exceptions: &[super::query_case::QueryException],
    lsp_timeout_seconds: u64,
    deadline_seconds: u64,
) -> Result<(), String> {
    validate_deadlines(label, lsp_timeout_seconds, deadline_seconds)?;
    let mut commands = BTreeSet::new();
    for exception in exceptions {
        if !commands.insert(exception.command) {
            return Err(format!("E2E smoke case {label} repeats an exception"));
        }
        require_text(&exception.reason, &format!("E2E exception for {label}"))?;
        match (&exception.outcome, &exception.message) {
            (ExceptionOutcome::Failure, Some(message)) => {
                require_text(message, &format!("E2E failure message for {label}"))?;
            }
            (ExceptionOutcome::Failure, None) => {
                return Err(format!(
                    "E2E failure exception for {label} must declare a message"
                ));
            }
            (ExceptionOutcome::EmptyMatches, Some(_)) => {
                return Err(format!(
                    "E2E empty-match exception for {label} must not declare a message"
                ));
            }
            (ExceptionOutcome::EmptyMatches, None) => {}
        }
    }
    Ok(())
}

fn validate_deadlines(
    label: &str,
    lsp_timeout_seconds: u64,
    deadline_seconds: u64,
) -> Result<(), String> {
    if lsp_timeout_seconds == 0 || deadline_seconds < lsp_timeout_seconds {
        Err(format!(
            "E2E smoke case {label} deadlines must be positive and ordered"
        ))
    } else {
        Ok(())
    }
}

pub(super) fn require_text(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} must be non-empty"))
    } else {
        Ok(())
    }
}
