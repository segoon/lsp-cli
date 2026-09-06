use std::collections::BTreeSet;
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
#[path = "manifest/dispositions.rs"]
mod dispositions;
use dispositions::require_text;

const MANIFEST_SCHEMA_VERSION: u32 = 6;

#[derive(Clone, Debug)]
pub(crate) struct Manifest {
    schema_version: u32,
    coverage: Coverage,
    commands: Vec<CommandCase>,
    languages: Vec<LanguageCase>,
    pairs: Vec<PairCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct SuiteFile {
    schema_version: u32,
    coverage: Coverage,
    commands: Vec<CommandCase>,
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
    setup: Option<ServerSetup>,
    smoke: Option<SmokeDisposition>,
    lifecycle: Option<LifecycleDisposition>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    deny_unknown_fields
)]
enum SmokeDisposition {
    Queries {
        symbol_query: String,
        callable_query: String,
        format_file: PathBuf,
        expected_names: Vec<String>,
        #[serde(default)]
        exceptions: Vec<QueryException>,
        lsp_timeout_seconds: u64,
        deadline_seconds: u64,
    },
    Excluded {
        reason: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct ServerSetup {
    provision: Provision,
    #[serde(default)]
    host_programs: Vec<HostProgram>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Provision {
    method: ProvisionMethod,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProvisionMethod {
    Download,
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
    BuildIndex,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct QueryException {
    command: QueryKind,
    outcome: ExceptionOutcome,
    message: Option<String>,
    reason: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ExceptionOutcome {
    EmptyMatches,
    Failure,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct HostProgram {
    name: String,
    resolve: Vec<String>,
}

pub(crate) struct RealServerCase<'a> {
    language: &'a LanguageCase,
    pair: &'a PairCase,
    setup: &'a ServerSetup,
    symbol_query: &'a str,
    callable_query: &'a str,
    format_file: &'a Path,
    expected_names: &'a [String],
    exceptions: &'a [QueryException],
    lsp_timeout_seconds: u64,
    deadline_seconds: u64,
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
            commands: suite.commands,
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
        let declared_pairs = self.validate_pairs(&data, &declared_languages)?;
        self.validate_preferred_servers(&data, &declared_pairs)?;

        if self.coverage == Coverage::Complete {
            Self::validate_complete_coverage(&data, &declared_languages, &declared_pairs)?;
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
                symbol_query,
                callable_query,
                format_file,
                expected_names,
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
            let setup = pair.setup.as_ref()?;
            Some(RealServerCase {
                language,
                pair,
                setup,
                symbol_query,
                callable_query,
                format_file,
                expected_names,
                exceptions,
                lsp_timeout_seconds: *lsp_timeout_seconds,
                deadline_seconds: *deadline_seconds,
            })
        })
    }

    pub(crate) fn real_server_lifecycle_cases(
        &self,
    ) -> impl Iterator<Item = RealServerLifecycleCase<'_>> {
        self.pairs
            .iter()
            .filter_map(|pair| RealServerLifecycleCase::from_pair(pair, &self.languages))
    }

    pub(crate) fn command_names(&self) -> BTreeSet<&str> {
        self.commands
            .iter()
            .map(|command| command.name.as_str())
            .collect()
    }

    pub(crate) fn declares_pair(&self, label: &str) -> bool {
        self.pairs
            .iter()
            .any(|pair| format!("{}/{}", pair.language, pair.server) == label)
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
        }
        Ok(declared)
    }

    fn validate_pairs(
        &self,
        data: &Path,
        declared_languages: &BTreeSet<String>,
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
            if let Some(smoke) = &pair.smoke {
                smoke.validate(pair)?;
            }
            if let Some(lifecycle) = &pair.lifecycle {
                lifecycle.validate(pair)?;
            }
            if matches!(pair.smoke, Some(SmokeDisposition::Queries { .. }))
                || matches!(pair.lifecycle, Some(LifecycleDisposition::Scenarios { .. }))
            {
                pair.setup
                    .as_ref()
                    .ok_or_else(|| {
                        format!(
                            "E2E executable pair {}/{} must declare server setup",
                            pair.language, pair.server
                        )
                    })?
                    .validate(pair)?;
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
        declared_pairs: &BTreeSet<PairKey>,
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

        let compatible = compatible_pairs(data, &detectable)?;
        let missing_pairs = compatible
            .difference(declared_pairs)
            .map(|pair| format!("{}/{}", pair.language, pair.server))
            .collect::<Vec<_>>();
        if !missing_pairs.is_empty() {
            return Err(format!(
                "complete E2E manifest is missing pairs: {}",
                missing_pairs.join(", ")
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
