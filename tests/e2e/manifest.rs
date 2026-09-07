use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::case_files::{file_stem, read_yaml, yaml_paths};
use crate::manifest_data::{
    FiletypeConfig, LspConfig, PairKey, compatible_pairs, detectable_languages, preferred_pairs,
};
use crate::repository_root;

#[path = "manifest/validation.rs"]
mod validation;
use validation::validate_config_id;
#[path = "manifest/lifecycle_case.rs"]
mod lifecycle_case;
#[path = "manifest/real_server_case.rs"]
mod real_server_case;
use lifecycle_case::LifecycleDisposition;
pub(crate) use lifecycle_case::RealServerLifecycleCase;
#[path = "manifest/provisioning_case.rs"]
mod provisioning_case;
pub(crate) use provisioning_case::ServerProvisioningCase;
use provisioning_case::{ProvisioningDisposition, ServerCase, setup_for_pair};
#[path = "manifest/query_case.rs"]
mod query_case;
pub(crate) use query_case::{
    ExceptionOutcome, QueryKind, RealServerCapabilitiesCase, RealServerCase,
};
use query_case::{HostProgram, QueryProfile, SmokeDisposition};
#[path = "manifest/dispositions.rs"]
mod dispositions;
use dispositions::require_text;
#[path = "manifest/coverage.rs"]
mod coverage_cases;

const MANIFEST_SCHEMA_VERSION: u32 = 8;

#[derive(Clone, Debug)]
pub(crate) struct Manifest {
    schema_version: u32,
    coverage: Coverage,
    platform: Platform,
    commands: Vec<CommandCase>,
    servers: Vec<ServerCase>,
    languages: Vec<LanguageCase>,
    pairs: Vec<PairCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct SuiteFile {
    schema_version: u32,
    coverage: Coverage,
    platform: Platform,
    commands: Vec<CommandCase>,
    servers: Vec<ServerCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct LanguageFile {
    language: LanguageCase,
    #[serde(default)]
    pairs: Vec<PairCase>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Coverage {
    Partial,
    Complete,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct CommandCase {
    name: String,
    strategy: CommandStrategy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CommandStrategy {
    Catalog,
    Filesystem,
    LspFixture,
    Lifecycle,
    UpdateFixture,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct LanguageCase {
    id: String,
    kind: ProjectKind,
    project: PathBuf,
    query_profile: Option<QueryProfile>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Platform {
    os: OperatingSystem,
    architecture: Architecture,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum OperatingSystem {
    Linux,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Architecture {
    X86_64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ProjectKind {
    Source,
    Metadata,
}

impl ProjectKind {
    fn label(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Metadata => "metadata",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct PairCase {
    language: String,
    server: String,
    smoke: Option<SmokeDisposition>,
    lifecycle: Option<LifecycleDisposition>,
}

impl Manifest {
    fn load() -> Result<Self, String> {
        Self::load_cases(repository_root())
    }

    fn load_cases(repository: &Path) -> Result<Self, String> {
        let directory = repository.join("tests/e2e/cases");
        let suite_path = directory.join("suite.yaml");
        let suite: SuiteFile = read_yaml(&suite_path)?;
        let mut languages = Vec::new();
        let mut pairs = Vec::new();

        for path in yaml_paths(&directory)? {
            if path == suite_path {
                continue;
            }
            let case_id = file_stem(&path)?;
            let case: LanguageFile = read_yaml(&path)?;
            let (language, mut language_pairs) = case.into_parts(&case_id, &path)?;
            languages.push(language);
            pairs.append(&mut language_pairs);
        }

        Ok(Self {
            schema_version: suite.schema_version,
            coverage: suite.coverage,
            platform: suite.platform,
            commands: suite.commands,
            servers: suite.servers,
            languages,
            pairs,
        })
    }

    fn validate(&self, repository: &Path) -> Result<(), String> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(format!(
                "unsupported E2E manifest schema version {}; expected {MANIFEST_SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.languages.is_empty() {
            return Err("E2E manifest must declare at least one language".to_string());
        }
        self.validate_commands()?;
        if self.pairs.is_empty() {
            return Err("E2E manifest must declare at least one language/server pair".to_string());
        }

        let data = repository.join("data");
        let declared_languages = self.validate_languages(repository, &data)?;
        let servers = self.validate_servers(&data)?;
        let declared_pairs = self.validate_pairs(&data, &declared_languages, &servers)?;
        self.validate_preferred_servers(&data, &declared_pairs)?;

        if self.coverage == Coverage::Complete {
            Self::validate_complete_coverage(&data, &declared_languages)?;
            self.validate_complete_pairs(&data, &servers, &declared_pairs)?;
        }
        Ok(())
    }

    pub(crate) fn load_validated(repository: &Path) -> Result<Self, String> {
        let manifest = Self::load_cases(repository)?;
        manifest.validate(repository)?;
        Ok(manifest)
    }

    pub(crate) fn load_repository() -> Result<Self, String> {
        Self::load_validated(repository_root())
    }

    pub(crate) fn real_server_smoke_cases(&self) -> impl Iterator<Item = RealServerCase<'_>> {
        self.pairs.iter().filter_map(|pair| {
            let SmokeDisposition::Queries {
                exceptions,
                lsp_timeout_seconds,
                deadline_seconds,
            } = pair.smoke.as_ref()?
            else {
                return None;
            };
            let language = self
                .languages
                .iter()
                .find(|language| language.id == pair.language)?;
            let profile = language.query_profile.as_ref()?;
            let setup = setup_for_pair(pair, &self.servers)?;
            Some(RealServerCase {
                language,
                pair,
                setup,
                symbol_query: &profile.symbol_query,
                callable_query: &profile.callable_query,
                format_file: &profile.format_file,
                expected_names: &profile.expected_names,
                exceptions,
                lsp_timeout_seconds: *lsp_timeout_seconds,
                deadline_seconds: *deadline_seconds,
            })
        })
    }

    pub(crate) fn real_server_lifecycle_cases(
        &self,
    ) -> impl Iterator<Item = RealServerLifecycleCase<'_>> {
        self.pairs.iter().filter_map(|pair| {
            RealServerLifecycleCase::from_pair(pair, &self.languages, &self.servers)
        })
    }

    pub(crate) fn server_provisioning_cases(
        &self,
    ) -> impl Iterator<Item = ServerProvisioningCase<'_>> {
        self.servers
            .iter()
            .filter_map(|server| ServerProvisioningCase::from_server(server, &self.languages))
    }

    pub(crate) fn command_names(&self) -> BTreeSet<&str> {
        self.commands
            .iter()
            .map(|command| command.name.as_str())
            .collect()
    }

    pub(crate) fn compatible_pair_inventory(
        &self,
        data: &Path,
    ) -> Result<BTreeSet<PairKey>, String> {
        let languages = self
            .languages
            .iter()
            .map(|language| language.id.clone())
            .collect();
        compatible_pairs(data, &languages)
    }

    pub(crate) fn declares_server(&self, id: &str) -> bool {
        self.servers.iter().any(|server| server.id == id)
    }

    pub(crate) fn server_is_downloadable(&self, id: &str) -> bool {
        self.servers
            .iter()
            .any(|server| server.id == id && server.is_downloadable())
    }

    pub(crate) fn commands_for(&self, strategy: CommandStrategy) -> impl Iterator<Item = &str> {
        self.commands
            .iter()
            .filter(move |command| command.strategy == strategy)
            .map(|command| command.name.as_str())
    }

    fn validate_commands(&self) -> Result<(), String> {
        if self.commands.is_empty() {
            return Err("E2E manifest must assign command coverage".to_string());
        }
        let mut names = BTreeSet::new();
        for command in &self.commands {
            validate_config_id("command", &command.name)?;
            if !names.insert(&command.name) {
                return Err(format!(
                    "E2E manifest assigns command {:?} more than once",
                    command.name
                ));
            }
        }
        Ok(())
    }

    fn validate_languages(
        &self,
        repository: &Path,
        data: &Path,
    ) -> Result<BTreeSet<String>, String> {
        let mut declared = BTreeSet::new();
        for language in &self.languages {
            validate_config_id("language", &language.id)?;
            if !declared.insert(language.id.clone()) {
                return Err(format!(
                    "E2E manifest declares language {:?} more than once",
                    language.id
                ));
            }

            let filetype_path = data.join("filetypes").join(format!("{}.yaml", language.id));
            let filetype: FiletypeConfig = read_yaml(&filetype_path)?;
            if !filetype.is_detectable() {
                return Err(format!(
                    "E2E language {:?} has no extension or filename-pattern detection rule",
                    language.id
                ));
            }
            language.validate_project(repository)?;
            if language.kind == ProjectKind::Source && language.query_profile.is_none() {
                return Err(format!(
                    "E2E source language {:?} requires a query profile",
                    language.id
                ));
            }
            if let Some(profile) = &language.query_profile {
                profile.validate(&language.id)?;
            }
        }
        Ok(declared)
    }

    fn validate_servers<'a>(
        &'a self,
        data: &Path,
    ) -> Result<BTreeMap<&'a str, &'a ServerCase>, String> {
        let languages = self
            .languages
            .iter()
            .map(|language| (language.id.as_str(), language))
            .collect::<BTreeMap<_, _>>();
        let detectable = languages.keys().map(|id| (*id).to_string()).collect();
        let compatible = compatible_pairs(data, &detectable)?;
        let expected = compatible
            .iter()
            .map(|pair| pair.server.as_str())
            .collect::<BTreeSet<_>>();
        let mut declared = BTreeMap::new();
        for server in &self.servers {
            server.validate(data, &languages, &compatible)?;
            if declared.insert(server.id.as_str(), server).is_some() {
                return Err(format!(
                    "E2E provisioning inventory declares server {:?} more than once",
                    server.id
                ));
            }
        }
        let actual = declared.keys().copied().collect::<BTreeSet<_>>();
        let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
        let extra = actual.difference(&expected).copied().collect::<Vec<_>>();
        if !missing.is_empty() || !extra.is_empty() {
            return Err(format!(
                "E2E provisioning inventory does not match compatible servers; missing: {missing:?}; extra: {extra:?}"
            ));
        }
        Ok(declared)
    }

    fn validate_pairs<'a>(
        &self,
        data: &Path,
        declared_languages: &BTreeSet<String>,
        servers: &BTreeMap<&'a str, &'a ServerCase>,
    ) -> Result<BTreeSet<PairKey>, String> {
        let mut declared = BTreeSet::new();
        for pair in &self.pairs {
            validate_config_id("server", &pair.server)?;
            if !declared_languages.contains(&pair.language) {
                return Err(format!(
                    "E2E pair {}/{} references an undeclared language",
                    pair.language, pair.server
                ));
            }
            if !declared.insert(pair.key()) {
                return Err(format!(
                    "E2E manifest declares pair {}/{} more than once",
                    pair.language, pair.server
                ));
            }

            let lsp_path = data.join("lsp").join(format!("{}.yaml", pair.server));
            let lsp: LspConfig = read_yaml(&lsp_path)?;
            if !lsp.filetypes.contains(&pair.language) {
                return Err(format!(
                    "LSP config {:?} does not support language {:?}",
                    pair.server, pair.language
                ));
            }
            if pair.smoke.is_none() && pair.lifecycle.is_none() {
                return Err(format!(
                    "E2E pair {}/{} must declare a smoke or lifecycle disposition",
                    pair.language, pair.server
                ));
            }
            if let Some(smoke) = &pair.smoke {
                smoke.validate(pair)?;
                let language = self
                    .languages
                    .iter()
                    .find(|item| item.id == pair.language)
                    .expect("declared language was checked above");
                match smoke {
                    SmokeDisposition::Queries { .. } if language.query_profile.is_none() => {
                        return Err(format!(
                            "E2E query pair {}/{} requires a language query profile",
                            pair.language, pair.server
                        ));
                    }
                    SmokeDisposition::Capabilities { .. }
                        if language.kind != ProjectKind::Metadata =>
                    {
                        return Err(format!(
                            "E2E capabilities-only pair {}/{} requires a metadata project",
                            pair.language, pair.server
                        ));
                    }
                    _ => {}
                }
            }
            if let Some(lifecycle) = &pair.lifecycle {
                lifecycle.validate(pair)?;
            }
            if matches!(
                pair.smoke,
                Some(SmokeDisposition::Queries { .. } | SmokeDisposition::Capabilities { .. })
            ) || matches!(pair.lifecycle, Some(LifecycleDisposition::Scenarios { .. }))
            {
                let Some(server) = servers.get(pair.server.as_str()) else {
                    return Err(format!(
                        "E2E executable pair {}/{} has no provisioning inventory entry",
                        pair.language, pair.server
                    ));
                };
                if !server.is_downloadable() {
                    return Err(format!(
                        "E2E executable pair {}/{} uses an excluded provisioning server",
                        pair.language, pair.server
                    ));
                }
            }
        }
        Ok(declared)
    }

    fn validate_preferred_servers(
        &self,
        data: &Path,
        declared_pairs: &BTreeSet<PairKey>,
    ) -> Result<(), String> {
        let source_languages = self
            .languages
            .iter()
            .filter(|language| language.kind == ProjectKind::Source)
            .map(|language| language.id.clone())
            .collect::<BTreeSet<_>>();
        let preferred = preferred_pairs(data, &source_languages)?;
        for pair in &preferred {
            if !declared_pairs.contains(pair) {
                return Err(format!(
                    "E2E source language {:?} is missing its data-preferred server pair {:?}",
                    pair.language, pair.server
                ));
            }
            let declared = self
                .pairs
                .iter()
                .find(|declared| declared.key() == *pair)
                .expect("declared pair was checked above");
            if declared.smoke.is_none() {
                return Err(format!(
                    "E2E preferred pair {}/{} must declare queries or an exclusion",
                    pair.language, pair.server
                ));
            }
        }
        for server in preferred
            .iter()
            .map(|pair| pair.server.as_str())
            .collect::<BTreeSet<_>>()
        {
            let owners = self
                .pairs
                .iter()
                .filter(|pair| pair.server == server && pair.lifecycle.is_some())
                .collect::<Vec<_>>();
            let [owner] = owners.as_slice() else {
                return Err(format!(
                    "preferred LSP server {server:?} must have exactly one lifecycle owner; found {}",
                    owners.len()
                ));
            };
            if !preferred.contains(&owner.key()) {
                return Err(format!(
                    "lifecycle owner {}/{} is not a preferred language/server pair",
                    owner.language, owner.server
                ));
            }
        }
        Ok(())
    }

    fn validate_complete_coverage(
        data: &Path,
        declared_languages: &BTreeSet<String>,
    ) -> Result<(), String> {
        let detectable = detectable_languages(data)?;
        let missing_languages = detectable
            .difference(declared_languages)
            .cloned()
            .collect::<Vec<_>>();
        if !missing_languages.is_empty() {
            return Err(format!(
                "complete E2E manifest is missing languages: {}",
                missing_languages.join(", ")
            ));
        }

        Ok(())
    }
}

impl LanguageFile {
    fn into_parts(
        self,
        case_id: &str,
        path: &Path,
    ) -> Result<(LanguageCase, Vec<PairCase>), String> {
        if self.language.id != case_id {
            return Err(format!(
                "E2E case filename {case_id:?} does not match language ID {:?} in {}",
                self.language.id,
                path.display()
            ));
        }
        if let Some(pair) = self
            .pairs
            .iter()
            .find(|pair| pair.language != self.language.id)
        {
            return Err(format!(
                "E2E case {} for language {:?} contains pair for language {:?}",
                path.display(),
                self.language.id,
                pair.language
            ));
        }
        Ok((self.language, self.pairs))
    }
}

impl PairCase {
    fn key(&self) -> PairKey {
        PairKey {
            language: self.language.clone(),
            server: self.server.clone(),
        }
    }
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
