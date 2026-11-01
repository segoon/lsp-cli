use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::*;

#[derive(Clone, Copy)]
pub enum WorkflowSelector {
    All,
    Language,
    Server,
    InstallationFamily,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallationFamily {
    Cargo,
    Generic,
    Github,
    Golang,
    Luarocks,
    Npm,
    Nuget,
    Pypi,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowPackage {
    pub installation_family: InstallationFamily,
    pub source_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RegistrySnapshot {
    pub release_tag: String,
    pub refreshed_at_epoch_seconds: u64,
    pub digest: Option<String>,
}

#[derive(Serialize)]
pub struct WorkflowPlan {
    families: BTreeMap<InstallationFamily, WorkflowFamily>,
}

#[derive(Serialize)]
struct WorkflowFamily {
    has_runnable: bool,
    matrix: WorkflowMatrix,
}

pub struct DownloadableServer {
    pub id: String,
    pub name: String,
    pub program: String,
}

#[derive(Serialize)]
struct WorkflowMatrix {
    include: Vec<WorkflowShard>,
}

#[derive(Serialize)]
struct WorkflowShard {
    name: String,
    language: String,
    installation_family: InstallationFamily,
    cases: String,
}

impl WorkflowPlan {
    fn from_shards(shards: impl IntoIterator<Item = WorkflowShard>) -> Result<Self, String> {
        let mut families: BTreeMap<_, _> = InstallationFamily::ALL
            .into_iter()
            .map(|family| {
                (
                    family,
                    WorkflowFamily {
                        has_runnable: false,
                        matrix: WorkflowMatrix {
                            include: Vec::new(),
                        },
                    },
                )
            })
            .collect();
        for shard in shards {
            let family = families
                .get_mut(&shard.installation_family)
                .expect("all installation families should have a matrix");
            family.has_runnable = true;
            family.matrix.include.push(shard);
        }
        for (family, plan) in &families {
            let count = plan.matrix.include.len();
            if count > 256 {
                return Err(format!(
                    "E2E installation family {:?} requires {count} jobs, exceeding GitHub Actions' limit of 256 jobs per matrix; split this family's CI matrix before running the workflow",
                    family.label()
                ));
            }
        }
        Ok(Self { families })
    }

    fn append_summary(&self, report: &mut String) {
        report.push_str("\n## CI matrices\n\n| Installation family | Jobs |\n| --- | --- |\n");
        for (family, plan) in &self.families {
            report.push_str(&format!(
                "| `{}` | {} |\n",
                family.label(),
                plan.matrix.include.len()
            ));
        }
    }
}

impl Manifest {
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
        let include = grouped.into_iter().map(|((language, family), pairs)| {
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
        });
        let plan = WorkflowPlan::from_shards(include)?;
        let mut report = self.workflow_report(&selected, packages, snapshot)?;
        plan.append_summary(&mut report);
        Ok((plan, report))
    }

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
}

impl InstallationFamily {
    const ALL: [Self; 8] = [
        Self::Cargo,
        Self::Generic,
        Self::Github,
        Self::Golang,
        Self::Luarocks,
        Self::Npm,
        Self::Nuget,
        Self::Pypi,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Generic => "generic",
            Self::Github => "github",
            Self::Golang => "golang",
            Self::Luarocks => "luarocks",
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
            "luarocks" => Ok(Self::Luarocks),
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

#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;
