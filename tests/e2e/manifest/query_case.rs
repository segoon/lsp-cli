use std::path::PathBuf;

use serde::Deserialize;

use super::{LanguageCase, PairCase, ServerCase};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct QueryProfile {
    pub(super) symbol_query: String,
    pub(super) callable_query: String,
    pub(super) format_file: PathBuf,
    pub(super) expected_names: Vec<String>,
}

impl QueryProfile {
    pub(super) fn validate(&self, language: &str) -> Result<(), String> {
        if self.symbol_query.trim().is_empty() || self.callable_query.trim().is_empty() {
            return Err(format!(
                "E2E query profile for {language:?} requires query terms"
            ));
        }
        if self.format_file.as_os_str().is_empty() || self.format_file.is_absolute() {
            return Err(format!("E2E format file for {language:?} must be relative"));
        }
        if self.expected_names.is_empty() || self.expected_names.iter().any(String::is_empty) {
            return Err(format!(
                "E2E query profile for {language:?} requires expected names"
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum SmokeDisposition {
    Queries {
        #[serde(default)]
        exceptions: Vec<QueryException>,
        lsp_timeout_seconds: Option<u64>,
        deadline_seconds: Option<u64>,
    },
    Capabilities {
        lsp_timeout_seconds: Option<u64>,
        deadline_seconds: Option<u64>,
    },
    Excluded {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QueryKind {
    ServerCapabilities,
    Diagnostics,
    Format,
    Grep,
    ListSymbols,
    ListFunctions,
    References,
    Callers,
    Callees,
    Definition,
    Declaration,
    BuildIndex,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct QueryException {
    pub(super) command: QueryKind,
    pub(super) outcome: ExceptionOutcome,
    pub(super) message: Option<String>,
    pub(super) reason: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ExceptionOutcome {
    EmptyMatches,
    Failure,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct HostProgram {
    pub(super) name: String,
    pub(super) resolve: Vec<String>,
}

pub(crate) struct RealServerCase<'a> {
    pub(super) language: &'a LanguageCase,
    pub(super) pair: &'a PairCase,
    pub(super) setup: &'a ServerCase,
    pub(super) symbol_query: &'a str,
    pub(super) callable_query: &'a str,
    pub(super) format_file: &'a std::path::Path,
    pub(super) expected_names: &'a [String],
    pub(super) exceptions: &'a [QueryException],
    pub(super) lsp_timeout_seconds: u64,
    pub(super) deadline_seconds: u64,
}

pub(crate) struct RealServerCapabilitiesCase<'a> {
    pub(super) language: &'a LanguageCase,
    pub(super) pair: &'a PairCase,
    pub(super) setup: &'a ServerCase,
    pub(super) lsp_timeout_seconds: u64,
    pub(super) deadline_seconds: u64,
}
