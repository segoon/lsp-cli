use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    coverage: Coverage,
    languages: Vec<LanguageCase>,
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
struct LanguageCase {
    id: String,
    kind: ProjectKind,
    project: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct PairCase {
    language: String,
    server: String,
}

#[derive(Deserialize)]
struct FiletypeConfig {
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    patterns: Vec<String>,
}

impl FiletypeConfig {
    fn is_detectable(&self) -> bool {
        !self.extensions.is_empty() || !self.patterns.is_empty()
    }
}

#[derive(Deserialize)]
struct LspConfig {
    #[serde(default)]
    filetypes: Vec<String>,
}

impl Manifest {
    fn load() -> Result<Self, String> {
        serde_yaml::from_str(include_str!("cases.yaml"))
            .map_err(|error| format!("failed to parse E2E manifest: {error}"))
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
        if self.pairs.is_empty() {
            return Err("E2E manifest must declare at least one language/server pair".to_string());
        }

        let data = repository.join("data");
        let declared_languages = self.validate_languages(repository, &data)?;
        let declared_pairs = self.validate_pairs(&data, &declared_languages)?;

        if self.coverage == Coverage::Complete {
            Self::validate_complete_coverage(&data, &declared_languages, &declared_pairs)?;
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
    ) -> Result<BTreeSet<PairCase>, String> {
        let mut declared = BTreeSet::new();
        for pair in &self.pairs {
            validate_config_id("server", &pair.server)?;
            if !declared_languages.contains(&pair.language) {
                return Err(format!(
                    "E2E pair {}/{} references an undeclared language",
                    pair.language, pair.server
                ));
            }
            if !declared.insert(pair.clone()) {
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
        }
        Ok(declared)
    }

    fn validate_complete_coverage(
        data: &Path,
        declared_languages: &BTreeSet<String>,
        declared_pairs: &BTreeSet<PairCase>,
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

fn detectable_languages(data: &Path) -> Result<BTreeSet<String>, String> {
    let mut languages = BTreeSet::new();
    for path in yaml_paths(&data.join("filetypes"))? {
        let config: FiletypeConfig = read_yaml(&path)?;
        if config.is_detectable() {
            languages.insert(file_stem(&path)?);
        }
    }
    Ok(languages)
}

fn compatible_pairs(
    data: &Path,
    detectable: &BTreeSet<String>,
) -> Result<BTreeSet<PairCase>, String> {
    let mut pairs = BTreeSet::new();
    for path in yaml_paths(&data.join("lsp"))? {
        let config: LspConfig = read_yaml(&path)?;
        let server = file_stem(&path)?;
        for language in config.filetypes {
            if detectable.contains(&language) {
                pairs.insert(PairCase {
                    language,
                    server: server.clone(),
                });
            }
        }
    }
    Ok(pairs)
}

fn yaml_paths(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("failed to read {}: {error}", directory.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().and_then(|extension| extension.to_str()) == Some("yaml"));
    paths.sort();
    Ok(paths)
}

fn read_yaml<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_yaml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn file_stem(path: &Path) -> Result<String, String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{} has no UTF-8 file stem", path.display()))
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

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn partial_manifest_matches_pinned_data() {
    Manifest::load()
        .expect("E2E manifest should parse")
        .validate(&repository_root())
        .expect("E2E manifest should be valid");
}

#[test]
fn complete_mode_rejects_the_partial_matrix() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest.coverage = Coverage::Complete;

    let error = manifest
        .validate(&repository_root())
        .expect_err("partial matrix should not satisfy complete coverage");

    assert!(error.contains("complete E2E manifest is missing languages"));
}

#[test]
fn complete_mode_rejects_missing_server_pairs() {
    let data = repository_root().join("data");
    let detectable = detectable_languages(&data).expect("filetype configs should load");
    let declared_pairs = BTreeSet::from([PairCase {
        language: "rust".to_string(),
        server: "rust_analyzer".to_string(),
    }]);

    let error = Manifest::validate_complete_coverage(&data, &detectable, &declared_pairs)
        .expect_err("partial server matrix should not satisfy complete coverage");

    assert!(error.contains("complete E2E manifest is missing pairs"));
}

#[test]
fn manifest_rejects_unknown_fields() {
    let error = serde_yaml::from_str::<Manifest>(
        "schema-version: 1\ncoverage: partial\nlanguages: []\npairs: []\nunknown: true\n",
    )
    .expect_err("unknown manifest fields should fail");

    assert!(error.to_string().contains("unknown field `unknown`"));
}

#[test]
fn manifest_rejects_config_path_traversal() {
    let mut manifest = Manifest::load().expect("E2E manifest should parse");
    manifest
        .languages
        .first_mut()
        .expect("manifest should contain a language")
        .id = "../rust".to_string();
    manifest
        .pairs
        .first_mut()
        .expect("manifest should contain a pair")
        .language = "../rust".to_string();

    let error = manifest
        .validate(&repository_root())
        .expect_err("config path traversal should fail");

    assert!(error.contains("must be one normalized path component"));
}
