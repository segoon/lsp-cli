use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::case_files::{file_stem, read_yaml, yaml_paths};
use crate::manifest_data::{
    FiletypeConfig, LspConfig, PairKey, compatible_pairs, detectable_languages,
};
use crate::repository_root;

const MANIFEST_SCHEMA_VERSION: u32 = 3;

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
    preferred: Option<PreferredServer>,
    smoke: Option<SmokeCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct PreferredServer {
    version: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct SmokeCase {
    provision: Provision,
    query: Query,
    expected_names: Vec<String>,
    #[serde(default)]
    host_programs: Vec<HostProgram>,
    lsp_timeout_seconds: u64,
    deadline_seconds: u64,
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

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Query {
    kind: QueryKind,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum QueryKind {
    ListSymbols,
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
    smoke: &'a SmokeCase,
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
        self.validate_preferred_servers()?;

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
            let smoke = pair.smoke.as_ref()?;
            let language = self
                .languages
                .iter()
                .find(|language| language.id == pair.language)?;
            Some(RealServerCase {
                language,
                pair,
                smoke,
            })
        })
    }

    pub(crate) fn command_names(&self) -> BTreeSet<&str> {
        self.commands
            .iter()
            .map(|command| command.name.as_str())
            .collect()
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
            if let Some(preferred) = &pair.preferred {
                preferred.validate(pair)?;
            }
        }
        Ok(declared)
    }

    fn validate_preferred_servers(&self) -> Result<(), String> {
        for language in &self.languages {
            let count = self
                .pairs
                .iter()
                .filter(|pair| pair.language == language.id && pair.preferred.is_some())
                .count();
            match (language.kind, count) {
                (ProjectKind::Source, 1) | (ProjectKind::Metadata, 0) => {}
                (ProjectKind::Source, _) => {
                    return Err(format!(
                        "E2E source language {:?} must select exactly one preferred server; found {count}",
                        language.id
                    ));
                }
                (ProjectKind::Metadata, _) => {
                    return Err(format!(
                        "E2E metadata language {:?} must not select a preferred server",
                        language.id
                    ));
                }
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

impl PreferredServer {
    fn validate(&self, pair: &PairCase) -> Result<(), String> {
        let version = self.version.trim();
        if version.is_empty() || version != self.version {
            return Err(format!(
                "preferred E2E server {}/{} must have a non-empty, trimmed version",
                pair.language, pair.server
            ));
        }
        if version.eq_ignore_ascii_case("latest") || version.eq_ignore_ascii_case("stable") {
            return Err(format!(
                "preferred E2E server {}/{} must use an exact version instead of {:?}",
                pair.language, pair.server, self.version
            ));
        }
        Ok(())
    }
}

impl SmokeCase {
    fn validate(&self, pair: &PairCase) -> Result<(), String> {
        let label = format!("{}/{}", pair.language, pair.server);
        if self.expected_names.is_empty() || self.expected_names.iter().any(String::is_empty) {
            return Err(format!(
                "E2E smoke case {label} must declare non-empty expected names"
            ));
        }
        if self.lsp_timeout_seconds == 0 || self.deadline_seconds == 0 {
            return Err(format!("E2E smoke case {label} deadlines must be positive"));
        }
        if self.deadline_seconds < self.lsp_timeout_seconds {
            return Err(format!(
                "E2E smoke case {label} deadline must not be shorter than its LSP timeout"
            ));
        }

        let mut names = BTreeSet::new();
        for program in &self.host_programs {
            validate_config_id("host program", &program.name)?;
            if !names.insert(&program.name) {
                return Err(format!(
                    "E2E smoke case {label} declares host program {:?} more than once",
                    program.name
                ));
            }
            if program.resolve.is_empty() || program.resolve.iter().any(String::is_empty) {
                return Err(format!(
                    "E2E host program {:?} for {label} must have a non-empty resolver command",
                    program.name
                ));
            }
        }
        Ok(())
    }
}

impl RealServerCase<'_> {
    pub(crate) fn label(&self) -> String {
        format!("{}/{}", self.pair.language, self.pair.server)
    }

    pub(crate) fn language(&self) -> &str {
        &self.language.id
    }

    pub(crate) fn server_name(&self, repository: &Path) -> Result<String, String> {
        let path = repository
            .join("data/lsp")
            .join(format!("{}.yaml", self.pair.server));
        let config: LspConfig = read_yaml(&path)?;
        Ok(config.name)
    }

    pub(crate) fn project(&self) -> &Path {
        &self.language.project
    }

    pub(crate) fn provision_method(&self) -> ProvisionMethod {
        self.smoke.provision.method
    }

    pub(crate) fn query_kind(&self) -> QueryKind {
        self.smoke.query.kind
    }

    pub(crate) fn expected_names(&self) -> &[String] {
        &self.smoke.expected_names
    }

    pub(crate) fn host_programs(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.smoke
            .host_programs
            .iter()
            .map(|program| (program.name.as_str(), program.resolve.as_slice()))
    }

    pub(crate) fn lsp_timeout_seconds(&self) -> u64 {
        self.smoke.lsp_timeout_seconds
    }

    pub(crate) fn deadline_seconds(&self) -> u64 {
        self.smoke.deadline_seconds
    }
}

impl LanguageCase {
    fn validate_project(&self, repository: &Path) -> Result<(), String> {
        if self.project.is_absolute()
            || self
                .project
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!(
                "E2E project path {} must be a normalized relative path",
                self.project.display()
            ));
        }

        let project = repository.join(&self.project);
        if !project.is_dir() {
            return Err(format!(
                "E2E {} project {} for language {:?} is not a directory",
                self.kind.label(),
                self.project.display(),
                self.id
            ));
        }
        let canonical_repository = repository
            .canonicalize()
            .map_err(|error| format!("failed to resolve {}: {error}", repository.display()))?;
        let canonical_project = project
            .canonicalize()
            .map_err(|error| format!("failed to resolve {}: {error}", project.display()))?;
        if !canonical_project.starts_with(canonical_repository) {
            return Err(format!(
                "E2E project {} resolves outside the repository",
                self.project.display()
            ));
        }
        Ok(())
    }
}

fn validate_config_id(kind: &str, value: &str) -> Result<(), String> {
    let mut components = Path::new(value).components();
    if value.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(format!(
            "E2E {kind} config ID {value:?} must be one normalized path component"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
