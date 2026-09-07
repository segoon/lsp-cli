use std::path::Path;

use serde::Deserialize;

use super::{LanguageCase, PairCase, ServerCase, require_text, setup_for_pair};

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum LifecycleDisposition {
    Scenarios {
        direct_run: DirectRunDisposition,
        lsp_timeout_seconds: u64,
        deadline_seconds: u64,
    },
    Excluded {
        reason: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum DirectRunDisposition {
    Exchange,
    Excluded { reason: String },
}

pub(crate) struct RealServerLifecycleCase<'a> {
    language: &'a LanguageCase,
    pair: &'a PairCase,
    setup: &'a ServerCase,
    direct_run: &'a DirectRunDisposition,
    lsp_timeout_seconds: u64,
    deadline_seconds: u64,
}

impl LifecycleDisposition {
    pub(super) fn validate(&self, pair: &PairCase) -> Result<(), String> {
        let label = format!("{}/{}", pair.language, pair.server);
        match self {
            Self::Excluded { reason } => {
                require_text(reason, &format!("E2E lifecycle exclusion for {label}"))
            }
            Self::Scenarios {
                direct_run,
                lsp_timeout_seconds,
                deadline_seconds,
            } => {
                if *lsp_timeout_seconds == 0 || *deadline_seconds < *lsp_timeout_seconds {
                    return Err(format!(
                        "E2E lifecycle case {label} deadlines must be positive and ordered"
                    ));
                }
                if let DirectRunDisposition::Excluded { reason } = direct_run {
                    require_text(reason, &format!("E2E direct-run exclusion for {label}"))?;
                }
                Ok(())
            }
        }
    }
}

impl<'a> RealServerLifecycleCase<'a> {
    pub(super) fn from_pair(
        pair: &'a PairCase,
        languages: &'a [LanguageCase],
        servers: &'a [ServerCase],
    ) -> Option<Self> {
        let LifecycleDisposition::Scenarios {
            direct_run,
            lsp_timeout_seconds,
            deadline_seconds,
        } = pair.lifecycle.as_ref()?
        else {
            return None;
        };
        Some(Self {
            language: languages
                .iter()
                .find(|language| language.id == pair.language)?,
            pair,
            setup: setup_for_pair(pair, servers)?,
            direct_run,
            lsp_timeout_seconds: *lsp_timeout_seconds,
            deadline_seconds: *deadline_seconds,
        })
    }

    pub(crate) fn label(&self) -> String {
        format!("{}/{}", self.pair.language, self.pair.server)
    }

    pub(crate) fn language(&self) -> &str {
        &self.language.id
    }

    pub(crate) fn server_name(&self, repository: &Path) -> Result<String, String> {
        super::real_server_case::server_name(&self.pair.server, repository)
    }

    pub(crate) fn project(&self) -> &Path {
        &self.language.project
    }

    pub(crate) fn host_programs(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.setup.host_programs()
    }

    pub(crate) fn direct_run_enabled(&self) -> bool {
        matches!(self.direct_run, DirectRunDisposition::Exchange)
    }

    pub(crate) fn lsp_timeout_seconds(&self) -> u64 {
        self.lsp_timeout_seconds
    }

    pub(crate) fn deadline_seconds(&self) -> u64 {
        self.deadline_seconds
    }
}
