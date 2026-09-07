use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;

use super::{HostProgram, LanguageCase, LspConfig, PairCase, read_yaml, require_text};
use crate::manifest_data::PairKey;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct ServerCase {
    pub(super) id: String,
    pub(super) owner_language: String,
    pub(super) provisioning: ProvisioningDisposition,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum ProvisioningDisposition {
    Download {
        #[serde(default)]
        host_programs: Vec<HostProgram>,
        deadline_seconds: u64,
    },
    Excluded {
        reason: String,
    },
}

pub(crate) struct ServerProvisioningCase<'a> {
    server: &'a ServerCase,
    language: &'a LanguageCase,
    host_programs: &'a [HostProgram],
    deadline_seconds: u64,
}

impl ServerCase {
    pub(super) fn validate(
        &self,
        data: &Path,
        languages: &BTreeMap<&str, &LanguageCase>,
        compatible: &BTreeSet<PairKey>,
    ) -> Result<(), String> {
        super::validate_config_id("server", &self.id)?;
        let Some(language) = languages.get(self.owner_language.as_str()) else {
            return Err(format!(
                "E2E provisioning server {:?} has unknown owner language {:?}",
                self.id, self.owner_language
            ));
        };
        let pair = PairKey {
            language: language.id.clone(),
            server: self.id.clone(),
        };
        if !compatible.contains(&pair) {
            return Err(format!(
                "E2E provisioning server {:?} is not compatible with owner language {:?}",
                self.id, self.owner_language
            ));
        }
        let lsp_path = data.join("lsp").join(format!("{}.yaml", self.id));
        let _: LspConfig = read_yaml(&lsp_path)?;

        match &self.provisioning {
            ProvisioningDisposition::Download {
                host_programs,
                deadline_seconds,
            } => {
                if *deadline_seconds == 0 {
                    return Err(format!(
                        "E2E provisioning deadline for {:?} must be positive",
                        self.id
                    ));
                }
                validate_host_programs(&self.id, host_programs)
            }
            ProvisioningDisposition::Excluded { reason } => require_text(
                reason,
                &format!("E2E provisioning exclusion for {:?}", self.id),
            ),
        }
    }

    pub(super) fn is_downloadable(&self) -> bool {
        matches!(self.provisioning, ProvisioningDisposition::Download { .. })
    }

    pub(super) fn host_programs(&self) -> impl Iterator<Item = (&str, &[String])> {
        let programs = match &self.provisioning {
            ProvisioningDisposition::Download { host_programs, .. } => host_programs.as_slice(),
            ProvisioningDisposition::Excluded { .. } => &[],
        };
        programs
            .iter()
            .map(|program| (program.name.as_str(), program.resolve.as_slice()))
    }
}

impl<'a> ServerProvisioningCase<'a> {
    pub(super) fn from_server(
        server: &'a ServerCase,
        languages: &'a [LanguageCase],
    ) -> Option<Self> {
        let ProvisioningDisposition::Download {
            host_programs,
            deadline_seconds,
        } = &server.provisioning
        else {
            return None;
        };
        Some(Self {
            server,
            language: languages
                .iter()
                .find(|language| language.id == server.owner_language)?,
            host_programs,
            deadline_seconds: *deadline_seconds,
        })
    }

    pub(crate) fn server_id(&self) -> &str {
        &self.server.id
    }

    pub(crate) fn language(&self) -> &str {
        &self.language.id
    }

    pub(crate) fn project(&self) -> &Path {
        &self.language.project
    }

    pub(crate) fn server_name(&self, repository: &Path) -> Result<String, String> {
        super::real_server_case::server_name(&self.server.id, repository)
    }

    pub(crate) fn host_programs(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.host_programs
            .iter()
            .map(|program| (program.name.as_str(), program.resolve.as_slice()))
    }

    pub(crate) fn deadline_seconds(&self) -> u64 {
        self.deadline_seconds
    }
}

pub(super) fn setup_for_pair<'a>(
    pair: &PairCase,
    servers: &'a [ServerCase],
) -> Option<&'a ServerCase> {
    servers
        .iter()
        .find(|server| server.id == pair.server && server.is_downloadable())
}

fn validate_host_programs(server: &str, programs: &[HostProgram]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for program in programs {
        super::validate_config_id("host program", &program.name)?;
        if !names.insert(&program.name) || program.resolve.is_empty() {
            return Err(format!(
                "E2E provisioning setup for {server:?} has an invalid host program"
            ));
        }
    }
    Ok(())
}
