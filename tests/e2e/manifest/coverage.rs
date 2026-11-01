use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::*;

impl Manifest {
    pub(crate) fn real_server_capabilities_cases(
        &self,
    ) -> impl Iterator<Item = RealServerCapabilitiesCase<'_>> {
        let pair_cases = self.pairs.iter().filter_map(|pair| {
            let SmokeDisposition::Capabilities {
                supported_operations,
                lsp_timeout_seconds,
                deadline_seconds,
            } = pair.smoke.as_ref()?
            else {
                return None;
            };
            let (lsp_timeout_seconds, deadline_seconds) = self
                .defaults
                .smoke
                .resolve(*lsp_timeout_seconds, *deadline_seconds);
            Some(RealServerCapabilitiesCase {
                language: self
                    .languages
                    .iter()
                    .find(|item| item.id == pair.language)?,
                setup: setup_for_pair(pair, &self.servers)?,
                supported_operations,
                smoke: pair.is_smoke(),
                lsp_timeout_seconds,
                deadline_seconds,
            })
        });
        let server_cases = self.servers.iter().filter_map(|server| {
            let (lsp_timeout_seconds, deadline_seconds) =
                server.capability_timeouts(self.defaults.smoke)?;
            Some(RealServerCapabilitiesCase {
                language: self
                    .languages
                    .iter()
                    .find(|item| item.id == server.owner_language)?,
                setup: server,
                supported_operations: server.capability_operations()?,
                smoke: false,
                lsp_timeout_seconds,
                deadline_seconds,
            })
        });
        pair_cases.chain(server_cases)
    }

    pub(crate) fn pair_server(&self, label: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|pair| pair.label() == label)
            .map(|pair| pair.server.as_str())
            .or_else(|| self.coverage_server_for_label(label))
    }

    pub(crate) fn smoke_servers(&self) -> BTreeSet<&str> {
        self.pairs
            .iter()
            .filter(|pair| pair.is_smoke())
            .map(|pair| pair.server.as_str())
            .collect()
    }

    pub(crate) fn excluded_behavior_count(
        &self,
        include: impl Fn(&str, &str, bool) -> bool,
    ) -> usize {
        self.pairs
            .iter()
            .filter(|pair| !pair.has_executable_behavior())
            .filter(|pair| include(&pair.label(), &pair.server, pair.is_smoke()))
            .count()
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

    pub(crate) fn declares_explicit_pair(&self, label: &str) -> bool {
        self.pairs
            .iter()
            .any(|pair| format!("{}/{}", pair.language, pair.server) == label)
            || self.coverage_server_for_label(label).is_some()
    }

    fn coverage_server_for_label(&self, label: &str) -> Option<&str> {
        self.servers
            .iter()
            .find(|server| {
                !server.requires_pair_coverage()
                    && format!("{}/{}", server.owner_language, server.id) == label
            })
            .map(|server| server.id.as_str())
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
            if item.owner_language == label.split_once('/')?.0
                && item.id == server
                && let super::provisioning_case::ServerCoverage::Unavailable { reason } =
                    &item.coverage
            {
                return Some(reason.clone());
            }
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
                servers.get(pair.server.as_str()).is_some_and(|server| {
                    server.is_downloadable() && server.requires_pair_coverage()
                })
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
