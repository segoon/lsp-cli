use std::collections::BTreeSet;

use super::{ExceptionOutcome, PairCase, ServerSetup, SmokeDisposition, validate_config_id};

impl SmokeDisposition {
    pub(super) fn validate(&self, pair: &PairCase) -> Result<(), String> {
        let label = format!("{}/{}", pair.language, pair.server);
        let Self::Queries {
            symbol_query,
            callable_query,
            format_file,
            expected_names,
            exceptions,
            lsp_timeout_seconds,
            deadline_seconds,
        } = self
        else {
            return match self {
                Self::Excluded { reason } => {
                    require_text(reason, &format!("E2E exclusion for {label}"))
                }
                Self::Queries { .. } => Ok(()),
            };
        };
        require_text(symbol_query, &format!("E2E symbol query for {label}"))?;
        require_text(callable_query, &format!("E2E callable query for {label}"))?;
        if format_file.as_os_str().is_empty() || format_file.is_absolute() {
            return Err(format!("E2E format file for {label} must be relative"));
        }
        if expected_names.is_empty() || expected_names.iter().any(String::is_empty) {
            return Err(format!(
                "E2E smoke case {label} must declare expected names"
            ));
        }
        if *lsp_timeout_seconds == 0 || *deadline_seconds < *lsp_timeout_seconds {
            return Err(format!(
                "E2E smoke case {label} deadlines must be positive and ordered"
            ));
        }
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
}

impl ServerSetup {
    pub(super) fn validate(&self, pair: &PairCase) -> Result<(), String> {
        let mut names = BTreeSet::new();
        for program in &self.host_programs {
            validate_config_id("host program", &program.name)?;
            if !names.insert(&program.name) || program.resolve.is_empty() {
                return Err(format!(
                    "E2E setup for {}/{} has an invalid host program",
                    pair.language, pair.server
                ));
            }
        }
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
