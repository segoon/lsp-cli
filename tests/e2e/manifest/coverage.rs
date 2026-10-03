use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[cfg(feature = "e2e-workflow-planner")]
use serde::{Deserialize, Serialize};

use super::*;

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Clone, Copy)]
pub enum WorkflowSelector {
    All,
    Language,
    Server,
    InstallationFamily,
}

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallationFamily {
    Cargo,
    Generic,
    Github,
    Golang,
    Npm,
    Nuget,
    Pypi,
}

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowPackage {
    pub installation_family: InstallationFamily,
    pub source_id: String,
}

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RegistrySnapshot {
    pub release_tag: String,
    pub refreshed_at_epoch_seconds: u64,
    pub digest: Option<String>,
}

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Serialize)]
pub struct WorkflowPlan {
    has_runnable: bool,
    matrix: WorkflowMatrix,
}

#[cfg(feature = "e2e-workflow-planner")]
pub struct DownloadableServer {
    pub id: String,
    pub name: String,
    pub program: String,
}

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Serialize)]
struct WorkflowMatrix {
    include: Vec<WorkflowShard>,
}

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Serialize)]
struct WorkflowShard {
    name: String,
    language: String,
    installation_family: InstallationFamily,
    cases: String,
}

impl Manifest {
    #[cfg(feature = "e2e-workflow-planner")]
    pub(crate) fn workflow_plan(
        &self,
        selector: WorkflowSelector,
        value: Option<&str>,
        packages: &BTreeMap<String, WorkflowPackage>,
        snapshot: &RegistrySnapshot,
    ) -> Result<(WorkflowPlan, String), String> {
        let selected = self.selected_workflow_pairs(selector, value, packages)?;
        let mut grouped = BTreeMap::<(String, InstallationFamily), Vec<&PairCase>>::new();
        for pair in &self.pairs {
            let key = pair.key();
            if !selected.contains(&key) || !pair.has_executable_behavior() {
                continue;
            }
            let server = self
                .servers
                .iter()
                .find(|server| server.id == pair.server)
                .expect("validated pair server should exist");
            let package = packages.get(&server.id).ok_or_else(|| {
                format!(
                    "Mason registry did not resolve installation family for {:?}",
                    server.id
                )
            })?;
            let family = package.installation_family;
            grouped
                .entry((pair.language.clone(), family))
                .or_default()
                .push(pair);
        }
        let include = grouped
            .into_iter()
            .map(|((language, family), pairs)| {
                let cases = pairs
                    .iter()
                    .map(|pair| format!("{}/{}", pair.language, pair.server))
                    .collect::<Vec<_>>()
                    .join(",");
                WorkflowShard {
                    name: format!("{language}/{}", family.label()),
                    language,
                    installation_family: family,
                    cases,
                }
            })
            .collect::<Vec<_>>();
        let report = self.workflow_report(&selected, packages, snapshot)?;
        Ok((
            WorkflowPlan {
                has_runnable: !include.is_empty(),
                matrix: WorkflowMatrix { include },
            },
            report,
        ))
    }

    #[cfg(feature = "e2e-workflow-planner")]
    fn selected_workflow_pairs(
        &self,
        selector: WorkflowSelector,
        value: Option<&str>,
        packages: &BTreeMap<String, WorkflowPackage>,
    ) -> Result<BTreeSet<PairKey>, String> {
        let compatible = self.compatible_pair_inventory(&repository_root().join("data"))?;
        let value = value.filter(|item| !item.trim().is_empty());
        let selected = match selector {
            WorkflowSelector::All if value.is_none() => compatible,
            WorkflowSelector::All => {
                return Err("the all selector does not accept a value".to_string());
            }
            WorkflowSelector::Language => {
                let value =
                    value.ok_or_else(|| "the language selector requires a value".to_string())?;
                if !self.languages.iter().any(|item| item.id == value) {
                    return Err(format!("unknown E2E language {value:?}"));
                }
                compatible
                    .into_iter()
                    .filter(|pair| pair.language == value)
                    .collect()
            }
            WorkflowSelector::Server => {
                let value =
                    value.ok_or_else(|| "the server selector requires a value".to_string())?;
                if !self.servers.iter().any(|item| item.id == value) {
                    return Err(format!("unknown E2E server {value:?}"));
                }
                compatible
                    .into_iter()
                    .filter(|pair| pair.server == value)
                    .collect()
            }
            WorkflowSelector::InstallationFamily => {
                let value = value.ok_or_else(|| {
                    "the installation-family selector requires a value".to_string()
                })?;
                let family = InstallationFamily::parse(value)?;
                compatible
                    .into_iter()
                    .filter(|pair| {
                        packages
                            .get(&pair.server)
                            .is_some_and(|package| package.installation_family == family)
                    })
                    .collect()
            }
        };
        Ok(selected)
    }

    #[cfg(feature = "e2e-workflow-planner")]
    pub(crate) fn downloadable_servers(&self) -> Result<Vec<DownloadableServer>, String> {
        self.servers
            .iter()
            .filter(|server| server.is_downloadable())
            .map(|server| {
                let path = repository_root()
                    .join("data/lsp")
                    .join(format!("{}.yaml", server.id));
                let config = read_yaml::<LspConfig>(&path)?;
                let program = config._cmdline.split_whitespace().next().ok_or_else(|| {
                    format!("LSP config {:?} has an empty command line", server.id)
                })?;
                Ok(DownloadableServer {
                    id: server.id.clone(),
                    name: config.name,
                    program: program.to_string(),
                })
            })
            .collect()
    }

    #[cfg(feature = "e2e-workflow-planner")]
    fn workflow_report(
        &self,
        selected: &BTreeSet<PairKey>,
        packages: &BTreeMap<String, WorkflowPackage>,
        snapshot: &RegistrySnapshot,
    ) -> Result<String, String> {
        let digest = snapshot.digest.as_deref().unwrap_or("unavailable");
        let mut report = format!(
            "## E2E compatibility report\n\nMason registry: `{}`; digest: `{digest}`; refreshed at Unix epoch `{}`.\n\n\
             | Pair | Classification | Installation family | Source ID | Reason |\n\
             | --- | --- | --- | --- | --- |\n",
            snapshot.release_tag, snapshot.refreshed_at_epoch_seconds
        );
        for pair in selected {
            let label = format!("{}/{}", pair.language, pair.server);
            let reason = self.exclusion_reason(&label);
            let classification = if reason.is_some() {
                "excluded"
            } else if self
                .servers
                .iter()
                .any(|server| server.id == pair.server && server.is_capabilities_only())
            {
                "capabilities-only"
            } else {
                "executable"
            };
            let family = packages
                .get(&pair.server)
                .map(|package| package.installation_family.label())
                .unwrap_or("unavailable");
            let source_id = packages
                .get(&pair.server)
                .map(|package| package.source_id.as_str())
                .unwrap_or("unavailable")
                .replace('|', "\\|");
            report.push_str(&format!(
                "| `{label}` | {classification} | `{family}` | `{source_id}` | {} |\n",
                reason.as_deref().unwrap_or("").replace('|', "\\|")
            ));
        }
        Ok(report)
    }

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

#[cfg(feature = "e2e-workflow-planner")]
impl InstallationFamily {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Generic => "generic",
            Self::Github => "github",
            Self::Golang => "golang",
            Self::Npm => "npm",
            Self::Nuget => "nuget",
            Self::Pypi => "pypi",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cargo" => Ok(Self::Cargo),
            "generic" => Ok(Self::Generic),
            "github" => Ok(Self::Github),
            "golang" => Ok(Self::Golang),
            "npm" => Ok(Self::Npm),
            "nuget" => Ok(Self::Nuget),
            "pypi" => Ok(Self::Pypi),
            _ => Err(format!("unknown E2E installation family {value:?}")),
        }
    }

    pub fn from_source_id(source_id: &str) -> Result<Self, String> {
        let family = source_id
            .strip_prefix("pkg:")
            .and_then(|value| value.split('/').next())
            .ok_or_else(|| format!("unsupported Mason source identifier {source_id:?}"))?;
        Self::parse(family)
    }
}

#[cfg(all(test, feature = "e2e-workflow-planner"))]
mod workflow_tests {
    use super::*;

    #[test]
    fn builds_serializable_shards_without_manifest_family_metadata() {
        let manifest = Manifest::load().expect("manifest should load");
        let families = manifest
            .downloadable_servers()
            .expect("downloadable servers should load")
            .into_iter()
            .map(|server| {
                assert!(!server.name.is_empty() && !server.program.is_empty());
                (
                    server.id,
                    WorkflowPackage {
                        installation_family: InstallationFamily::Github,
                        source_id: "pkg:github/example/server@v1.0.0".to_string(),
                    },
                )
            })
            .collect();
        let snapshot = RegistrySnapshot {
            release_tag: "2026-09-25".to_string(),
            refreshed_at_epoch_seconds: 1_797_000_000,
            digest: Some("sha256:0123".to_string()),
        };

        let (plan, report) = manifest
            .workflow_plan(WorkflowSelector::All, None, &families, &snapshot)
            .expect("workflow plan should build");

        assert!(serde_json::to_value(plan).expect("plan should serialize")["has_runnable"] == true);
        assert!(report.lines().any(|line| line.contains("executable")));
        assert!(report.contains("pkg:github/example/server@v1.0.0"));
        assert!(report.contains("2026-09-25"));
        assert!(report.contains("sha256:0123"));
        assert_eq!(
            InstallationFamily::from_source_id("pkg:golang/example/tool")
                .expect("known family should parse"),
            InstallationFamily::Golang
        );
        let _selectors = [
            WorkflowSelector::Language,
            WorkflowSelector::Server,
            WorkflowSelector::InstallationFamily,
        ];
    }
}
