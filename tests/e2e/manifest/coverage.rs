use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::*;

impl Manifest {
    pub(crate) fn real_server_capabilities_cases(
        &self,
    ) -> impl Iterator<Item = RealServerCapabilitiesCase<'_>> {
        self.pairs.iter().filter_map(|pair| {
            let SmokeDisposition::Capabilities {
                lsp_timeout_seconds,
                deadline_seconds,
            } = pair.smoke.as_ref()?
            else {
                return None;
            };
            Some(RealServerCapabilitiesCase {
                language: self
                    .languages
                    .iter()
                    .find(|item| item.id == pair.language)?,
                pair,
                setup: setup_for_pair(pair, &self.servers)?,
                lsp_timeout_seconds: *lsp_timeout_seconds,
                deadline_seconds: *deadline_seconds,
            })
        })
    }

    pub(crate) fn declares_pair(&self, label: &str) -> bool {
        if self
            .pairs
            .iter()
            .any(|pair| format!("{}/{}", pair.language, pair.server) == label)
        {
            return true;
        }
        let Some((language, server)) = label.split_once('/') else {
            return false;
        };
        if !self.languages.iter().any(|item| item.id == language)
            || !self.servers.iter().any(|item| {
                item.id == server
                    && matches!(item.provisioning, ProvisioningDisposition::Excluded { .. })
            })
        {
            return false;
        }
        let path = repository_root()
            .join("data/lsp")
            .join(format!("{server}.yaml"));
        read_yaml::<LspConfig>(&path)
            .is_ok_and(|config| config.filetypes.iter().any(|item| item == language))
    }

    pub(crate) fn platform_label(&self) -> &'static str {
        match (self.platform.os, self.platform.architecture) {
            (OperatingSystem::Linux, Architecture::X86_64) => "linux/x86_64",
        }
    }

    pub(crate) fn supports_current_platform(&self) -> bool {
        matches!(self.platform.os, OperatingSystem::Linux)
            && std::env::consts::OS == "linux"
            && matches!(self.platform.architecture, Architecture::X86_64)
            && std::env::consts::ARCH == "x86_64"
    }

    pub(crate) fn is_preferred_pair(&self, label: &str) -> bool {
        let languages = self
            .languages
            .iter()
            .filter(|language| language.kind == ProjectKind::Source)
            .map(|language| language.id.clone())
            .collect();
        preferred_pairs(&repository_root().join("data"), &languages).is_ok_and(|pairs| {
            pairs
                .iter()
                .any(|pair| format!("{}/{}", pair.language, pair.server) == label)
        })
    }

    pub(crate) fn exclusion_reason(&self, label: &str) -> Option<String> {
        if let Some(reason) = self.pairs.iter().find_map(|pair| {
            (format!("{}/{}", pair.language, pair.server) == label)
                .then_some(pair.smoke.as_ref())
                .flatten()
                .and_then(|smoke| match smoke {
                    SmokeDisposition::Excluded { reason } => Some(reason.clone()),
                    _ => None,
                })
        }) {
            return Some(reason);
        }
        let (_, server) = label.split_once('/')?;
        self.servers.iter().find_map(|item| {
            (item.id == server && self.declares_pair(label))
                .then_some(&item.provisioning)
                .and_then(|provisioning| match provisioning {
                    ProvisioningDisposition::Excluded { reason } => Some(reason.clone()),
                    ProvisioningDisposition::Download { .. } => None,
                })
        })
    }

    pub(super) fn validate_complete_pairs(
        &self,
        data: &Path,
        servers: &BTreeMap<&str, &ServerCase>,
        declared: &BTreeSet<PairKey>,
    ) -> Result<(), String> {
        let compatible = self.compatible_pair_inventory(data)?;
        let missing = compatible
            .iter()
            .filter(|pair| {
                servers
                    .get(pair.server.as_str())
                    .is_some_and(|server| server.is_downloadable())
            })
            .filter(|pair| !declared.contains(*pair))
            .map(|pair| format!("{}/{}", pair.language, pair.server))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "complete E2E manifest is missing downloadable pairs: {}",
                missing.join(", ")
            ))
        }
    }
}
