use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use super::{LanguageCase, PairCase, ServerCase};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct QueryProfile {
    pub(super) symbol_query: String,
    pub(super) callable_query: String,
    #[serde(default)]
    pub(super) command_queries: BTreeMap<QueryKind, String>,
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
        for (command, query) in &self.command_queries {
            if !command.accepts_query_override() {
                return Err(format!(
                    "E2E query profile for {language:?} cannot override {command:?}"
                ));
            }
            if query.trim().is_empty() {
                return Err(format!(
                    "E2E query profile for {language:?} requires a non-empty {command:?} query"
                ));
            }
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

    fn query_for(&self, command: QueryKind) -> &str {
        select_query(
            command,
            &self.symbol_query,
            &self.callable_query,
            &self.command_queries,
        )
    }
}

pub(super) fn select_query<'a>(
    command: QueryKind,
    symbol: &'a str,
    callable: &'a str,
    overrides: &'a BTreeMap<QueryKind, String>,
) -> &'a str {
    overrides.get(&command).map_or_else(
        || match command {
            QueryKind::Grep => symbol,
            _ => callable,
        },
        String::as_str,
    )
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
    Implementation,
    TypeDefinition,
    BuildIndex,
}

impl QueryKind {
    fn accepts_query_override(self) -> bool {
        matches!(
            self,
            Self::Grep
                | Self::References
                | Self::Callers
                | Self::Callees
                | Self::Definition
                | Self::Declaration
                | Self::Implementation
                | Self::TypeDefinition
        )
    }
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
    VariableMatches,
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
    pub(super) command_queries: &'a BTreeMap<QueryKind, String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(command: QueryKind, query: &str) -> QueryProfile {
        QueryProfile {
            symbol_query: "symbol".to_string(),
            callable_query: "callable".to_string(),
            command_queries: [(command, query.to_string())].into(),
            format_file: PathBuf::from("source.test"),
            expected_names: vec!["symbol".to_string()],
        }
    }

    #[test]
    fn selects_command_queries_with_compatible_fallbacks() {
        let profile = profile(QueryKind::Implementation, "implementation");

        assert_eq!(profile.query_for(QueryKind::Grep), "symbol");
        assert_eq!(profile.query_for(QueryKind::References), "callable");
        assert_eq!(
            profile.query_for(QueryKind::Implementation),
            "implementation"
        );
    }

    #[test]
    fn validates_command_query_keys_and_values() {
        for (command, query, expected) in [
            (QueryKind::BuildIndex, "invalid", "cannot override"),
            (QueryKind::Implementation, " ", "non-empty"),
        ] {
            let error = profile(command, query)
                .validate("test")
                .expect_err("invalid command query should fail");
            assert!(error.contains(expected), "unexpected error: {error}");
        }
    }
}
