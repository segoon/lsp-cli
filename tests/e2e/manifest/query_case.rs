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
        validate_expected_names(
            &self.expected_names,
            &format!("E2E query profile for {language:?}"),
        )
    }

    fn query_for(&self, command: QueryKind) -> &str {
        select_query(
            command,
            &self.symbol_query,
            &self.callable_query,
            &self.command_queries,
        )
    }

    pub(super) fn resolved_callable_query<'a>(&'a self, override_: Option<&'a str>) -> &'a str {
        select_callable_query(override_, &self.callable_query)
    }

    pub(super) fn resolved_expected_names<'a>(
        &'a self,
        override_: Option<&'a [String]>,
    ) -> &'a [String] {
        select_expected_names(override_, &self.expected_names)
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

pub(super) fn select_expected_names<'a>(
    override_: Option<&'a [String]>,
    defaults: &'a [String],
) -> &'a [String] {
    override_.unwrap_or(defaults)
}

pub(super) fn select_callable_query<'a>(override_: Option<&'a str>, default: &'a str) -> &'a str {
    override_.unwrap_or(default)
}

pub(super) fn validate_callable_query(query: Option<&str>, label: &str) -> Result<(), String> {
    if query.is_some_and(|query| query.trim().is_empty()) {
        Err(format!("{label} requires a non-empty callable query"))
    } else {
        Ok(())
    }
}

pub(super) fn validate_expected_names(names: &[String], label: &str) -> Result<(), String> {
    if names.is_empty() || names.iter().any(String::is_empty) {
        Err(format!("{label} requires expected names"))
    } else {
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
        callable_query: Option<String>,
        expected_names: Option<Vec<String>>,
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

    #[test]
    fn selects_and_validates_pair_expected_names() {
        let defaults = vec!["default".to_string()];
        let override_ = vec!["decorated()".to_string()];

        assert_eq!(select_expected_names(None, &defaults), defaults);
        assert_eq!(
            select_expected_names(Some(&override_), &defaults),
            override_
        );
        assert!(validate_expected_names(&override_, "pair").is_ok());
        for invalid in [Vec::new(), vec![String::new()]] {
            assert!(validate_expected_names(&invalid, "pair").is_err());
        }
        assert_eq!(select_callable_query(None, "default"), "default");
        assert_eq!(
            select_callable_query(Some("decorated()"), "default"),
            "decorated()"
        );
        assert!(validate_callable_query(Some(" "), "pair").is_err());
    }
}
