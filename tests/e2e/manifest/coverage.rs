use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[cfg(feature = "e2e-workflow-planner")]
use serde::Serialize;

use super::*;

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Clone, Copy)]
pub(crate) enum WorkflowSelector {
    All,
    Language,
    Server,
    InstallationFamily,
}

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum InstallationFamily {
    Cargo,
    Generic,
    Github,
    Golang,
    Npm,
    Nuget,
    Pypi,
}

#[cfg(feature = "e2e-workflow-planner")]
#[derive(Serialize)]
pub(crate) struct WorkflowPlan {
    has_runnable: bool,
    matrix: WorkflowMatrix,
}

#[cfg(feature = "e2e-workflow-planner")]
pub(crate) struct DownloadableServer {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) program: String,
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
    needs_go: bool,
    needs_java: bool,
    needs_node: bool,
    needs_dotnet: bool,
}

impl Manifest {
    #[cfg(feature = "e2e-workflow-planner")]
    pub(crate) fn workflow_plan(
        &self,
        selector: WorkflowSelector,
        value: Option<&str>,
        families: &BTreeMap<String, InstallationFamily>,
    ) -> Result<(WorkflowPlan, String), String> {
        let selected = self.selected_workflow_pairs(selector, value, families)?;
        let mut grouped = BTreeMap::<(String, InstallationFamily), Vec<&PairCase>>::new();
        for pair in &self.pairs {
            let key = pair.key();
            if !selected.contains(&key) {
                continue;
            }
            let server = self
                .servers
                .iter()
                .find(|server| server.id == pair.server)
                .expect("validated pair server should exist");
            let family = *families.get(&server.id).ok_or_else(|| {
                format!(
                    "Mason registry did not resolve installation family for {:?}",
                    server.id
                )
            })?;
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
                let programs = pairs
                    .iter()
                    .filter_map(|pair| self.servers.iter().find(|item| item.id == pair.server))
                    .flat_map(ServerCase::host_programs)
                    .map(|(name, _)| name)
                    .collect::<BTreeSet<_>>();
                WorkflowShard {
                    name: format!("{language}/{}", family.label()),
                    language,
                    installation_family: family,
                    cases,
                    needs_go: family == InstallationFamily::Golang || programs.contains("go"),
                    needs_java: programs.contains("java"),
                    needs_node: family == InstallationFamily::Npm
                        || programs.contains("node")
                        || programs.contains("npm"),
                    needs_dotnet: family == InstallationFamily::Nuget
                        || programs.contains("dotnet"),
                }
            })
            .collect::<Vec<_>>();
        let report = self.workflow_report(&selected)?;
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
        families: &BTreeMap<String, InstallationFamily>,
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
                    .filter(|pair| families.get(&pair.server).copied() == Some(family))
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
    fn workflow_report(&self, selected: &BTreeSet<PairKey>) -> Result<String, String> {
        let mut report = String::from(
            "## E2E compatibility report\n\n| Pair | Classification | Reason |\n| --- | --- | --- |\n",
        );
        for pair in selected {
            let label = format!("{}/{}", pair.language, pair.server);
            let reason = self.exclusion_reason(&label);
            let classification = if reason.is_some() {
                "excluded"
            } else {
                "executable"
            };
            report.push_str(&format!(
                "| `{label}` | {classification} | {} |\n",
                reason.as_deref().unwrap_or("").replace('|', "\\|")
            ));
        }
        Ok(report)
    }

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
            let (lsp_timeout_seconds, deadline_seconds) = self
                .defaults
                .smoke
                .resolve(*lsp_timeout_seconds, *deadline_seconds);
            Some(RealServerCapabilitiesCase {
                language: self
                    .languages
                    .iter()
                    .find(|item| item.id == pair.language)?,
                pair,
                setup: setup_for_pair(pair, &self.servers)?,
                lsp_timeout_seconds,
                deadline_seconds,
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

    pub(crate) fn declares_explicit_pair(&self, label: &str) -> bool {
        self.pairs
            .iter()
            .any(|pair| format!("{}/{}", pair.language, pair.server) == label)
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

    pub(crate) fn from_source_id(source_id: &str) -> Result<Self, String> {
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
                (server.id, InstallationFamily::Github)
            })
            .collect();

        let (plan, report) = manifest
            .workflow_plan(WorkflowSelector::All, None, &families)
            .expect("workflow plan should build");

        assert!(serde_json::to_value(plan).expect("plan should serialize")["has_runnable"] == true);
        assert!(report.lines().any(|line| line.contains("executable")));
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
