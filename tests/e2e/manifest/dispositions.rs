use std::collections::BTreeSet;

use super::{
    ExceptionOutcome, PairCase, SmokeDisposition,
    query_case::{validate_callable_query, validate_expected_names},
    suite::Timeouts,
};

impl SmokeDisposition {
    pub(super) fn validate(&self, pair: &PairCase, defaults: Timeouts) -> Result<(), String> {
        let label = format!("{}/{}", pair.language, pair.server);
        match self {
            Self::Excluded { reason } => {
                require_text(reason, &format!("E2E exclusion for {label}"))
            }
            Self::Capabilities {
                supported_operations,
                lsp_timeout_seconds,
                deadline_seconds,
            } => {
                let (lsp, deadline) = defaults.resolve(*lsp_timeout_seconds, *deadline_seconds);
                validate_operation_list(&label, supported_operations)?;
                validate_deadlines(&label, lsp, deadline)
            }
            Self::Queries {
                callable_query,
                expected_names,
                supported_operations,
                exceptions,
                lsp_timeout_seconds,
                deadline_seconds,
            } => {
                let (lsp, deadline) = defaults.resolve(*lsp_timeout_seconds, *deadline_seconds);
                if let Some(names) = expected_names {
                    validate_expected_names(names, &format!("E2E smoke case {label}"))?;
                }
                validate_callable_query(
                    callable_query.as_deref(),
                    &format!("E2E smoke case {label}"),
                )?;
                validate_queries(&label, supported_operations, exceptions, lsp, deadline)
            }
        }
    }
}

fn validate_queries(
    label: &str,
    supported_operations: &[super::QueryKind],
    exceptions: &[super::query_case::QueryException],
    lsp_timeout_seconds: u64,
    deadline_seconds: u64,
) -> Result<(), String> {
    validate_deadlines(label, lsp_timeout_seconds, deadline_seconds)?;
    let supported = validate_operation_list(label, supported_operations)?;
    for required in [super::QueryKind::Diagnostics, super::QueryKind::BuildIndex] {
        if !supported.contains(&required) {
            return Err(format!(
                "E2E smoke case {label} must support {}",
                required.command_name()
            ));
        }
    }
    let mut commands = BTreeSet::new();
    for exception in exceptions {
        if !supported.contains(&exception.command) {
            return Err(format!(
                "E2E exception for {label} requires its operation to be supported"
            ));
        }
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
            (ExceptionOutcome::EmptyMatches | ExceptionOutcome::VariableMatches, Some(_)) => {
                return Err(format!(
                    "E2E match-result exception for {label} must not declare a message"
                ));
            }
            (ExceptionOutcome::EmptyMatches | ExceptionOutcome::VariableMatches, None) => {}
        }
    }
    Ok(())
}

pub(super) fn validate_operation_list(
    label: &str,
    operations: &[super::QueryKind],
) -> Result<BTreeSet<super::QueryKind>, String> {
    let supported = operations.iter().copied().collect::<BTreeSet<_>>();
    if supported.len() != operations.len() {
        return Err(format!(
            "E2E smoke case {label} repeats a supported operation"
        ));
    }
    if !supported.contains(&super::QueryKind::ServerCapabilities) {
        return Err(format!(
            "E2E smoke case {label} must support server-capabilities"
        ));
    }
    Ok(supported)
}

pub(super) fn validate_deadlines(
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
