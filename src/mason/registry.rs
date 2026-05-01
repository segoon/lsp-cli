use crate::runtime_state::RuntimeState;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;
use zip::ZipArchive;

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/mason-org/mason-registry/releases/latest";
const REGISTRY_ASSET_NAME: &str = "registry.json.zip";
const REGISTRY_FRESHNESS_THRESHOLD: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MasonRegistry {
    lspconfigs: BTreeMap<String, MasonPackage>,
    package_names: BTreeMap<String, MasonPackage>,
    binaries: BTreeMap<String, String>,
}

impl MasonRegistry {
    pub fn load(state: &RuntimeState) -> Result<Self, String> {
        state.ensure_dirs()?;

        let registry_json_path = state.registry_json_path();
        match ensure_registry_cache(state) {
            Ok(()) => Self::from_registry_json_path(&registry_json_path),
            Err(error) if registry_json_path.is_file() => {
                eprintln!("warning: failed to refresh Mason registry, using cached data: {error}");
                Self::from_registry_json_path(&registry_json_path)
            }
            Err(error) => Err(error),
        }
    }

    #[must_use]
    pub fn load_cached(state: &RuntimeState) -> Option<Self> {
        let path = state.registry_json_path();
        path.is_file()
            .then_some(())
            .and_then(|()| Self::from_registry_json_path(&path).ok())
    }

    pub fn package_for_lspconfig(&self, lspconfig: &str) -> Option<&MasonPackage> {
        self.lspconfigs.get(lspconfig)
    }

    pub fn package_for_detected(
        &self,
        config_id: &str,
        server: &str,
        program: &str,
    ) -> Option<&MasonPackage> {
        self.package_for_lspconfig(config_id)
            .or_else(|| {
                mapping_override(config_id).and_then(|target| self.package_for_override(target))
            })
            .or_else(|| self.package_for_name(config_id))
            .or_else(|| self.package_for_name(server))
            .or_else(|| self.package_for_bin(program))
    }

    fn package_for_name(&self, package_name: &str) -> Option<&MasonPackage> {
        self.package_names.get(package_name)
    }

    fn package_for_bin(&self, program: &str) -> Option<&MasonPackage> {
        self.binaries
            .get(program)
            .and_then(|package_name| self.package_for_name(package_name))
    }

    fn package_for_override(&self, target: MappingOverride) -> Option<&MasonPackage> {
        match target {
            MappingOverride::Lspconfig(lspconfig) => self.package_for_lspconfig(lspconfig),
            MappingOverride::Package(package_name) => self.package_for_name(package_name),
        }
    }

    fn from_registry_json_path(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let package_values = serde_json::from_str::<Vec<serde_json::Value>>(&contents)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        let mut packages = Vec::new();
        for value in package_values.into_iter().filter(is_lsp_package_value) {
            if let Ok(mut package) = serde_json::from_value::<MasonPackage>(value) {
                let _ = package.apply_source_version_overrides();
                packages.push(package);
            }
        }

        if packages.is_empty() {
            return Err(format!(
                "failed to parse any Mason LSP packages from {}",
                path.display()
            ));
        }

        Ok(Self::from_packages(packages))
    }

    fn from_packages(packages: Vec<MasonPackage>) -> Self {
        let mut lspconfigs = BTreeMap::new();
        let mut package_names = BTreeMap::new();
        let mut binaries = BTreeMap::new();
        let mut ambiguous_bins = std::collections::BTreeSet::new();

        for mut package in packages {
            if !package.is_lsp() {
                continue;
            }

            let _ = package.apply_source_version_overrides();

            let package_name = package.name.clone();

            for binary in package.bin.keys() {
                if let Some(existing) = binaries.get(binary) {
                    if existing != &package_name {
                        ambiguous_bins.insert(binary.clone());
                    }
                } else {
                    binaries.insert(binary.clone(), package_name.clone());
                }
            }

            package_names.insert(package_name, package.clone());

            let Some(lspconfig) = package.neovim.lspconfig.clone() else {
                continue;
            };
            lspconfigs.insert(lspconfig, package);
        }

        for binary in ambiguous_bins {
            binaries.remove(&binary);
        }

        Self {
            lspconfigs,
            package_names,
            binaries,
        }
    }
}

#[derive(Clone, Copy)]
enum MappingOverride {
    Lspconfig(&'static str),
    Package(&'static str),
}

fn mapping_override(config_id: &str) -> Option<MappingOverride> {
    match config_id {
        // Conservative historical aliases that are still common in user config.
        "sumneko_lua" => Some(MappingOverride::Lspconfig("lua_ls")),
        "tsserver" => Some(MappingOverride::Lspconfig("ts_ls")),
        "typescript_language_server" => {
            Some(MappingOverride::Package("typescript-language-server"))
        }
        "volar" => Some(MappingOverride::Lspconfig("vue_ls")),
        _ => None,
    }
}

fn is_lsp_package_value(value: &serde_json::Value) -> bool {
    value
        .get("categories")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|categories| {
            categories
                .iter()
                .any(|category| category.as_str() == Some("LSP"))
        })
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonPackage {
    pub name: String,
    #[serde(default)]
    pub categories: Vec<String>,
    pub source: MasonSource,
    #[serde(default)]
    pub bin: BTreeMap<String, String>,
    #[serde(default)]
    pub share: BTreeMap<String, String>,
    #[serde(default)]
    pub neovim: MasonNeovim,
}

impl MasonPackage {
    fn is_lsp(&self) -> bool {
        self.categories.iter().any(|category| category == "LSP")
    }

    fn apply_source_version_overrides(&mut self) -> Result<(), String> {
        self.source.apply_version_overrides()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonSource {
    pub id: String,
    #[serde(default)]
    pub extra_packages: Vec<String>,
    #[serde(default)]
    pub asset: Option<OneOrMany<MasonAsset>>,
    #[serde(default)]
    pub download: Option<OneOrMany<MasonDownload>>,
    #[serde(default)]
    pub version_overrides: Vec<MasonVersionOverride>,
}

impl MasonSource {
    #[must_use]
    pub fn assets(&self) -> &[MasonAsset] {
        self.asset.as_ref().map_or(&[], OneOrMany::as_slice)
    }

    #[must_use]
    pub fn downloads(&self) -> &[MasonDownload] {
        self.download.as_ref().map_or(&[], OneOrMany::as_slice)
    }

    fn apply_version_overrides(&mut self) -> Result<(), String> {
        let version = source_id_version(&self.id)?;
        let override_ = self
            .version_overrides
            .iter()
            .filter(|override_| version_matches_constraint(version, &override_.constraint))
            .min_by(|left, right| {
                compare_versions(
                    constraint_upper_bound(&left.constraint),
                    constraint_upper_bound(&right.constraint),
                )
            })
            .cloned();

        let Some(override_) = override_ else {
            return Ok(());
        };

        self.id = override_.id;
        if let Some(extra_packages) = override_.extra_packages {
            self.extra_packages = extra_packages;
        }
        if let Some(asset) = override_.asset {
            self.asset = Some(asset);
        }
        if let Some(download) = override_.download {
            self.download = Some(download);
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonVersionOverride {
    pub constraint: String,
    pub id: String,
    #[serde(default)]
    pub extra_packages: Option<Vec<String>>,
    #[serde(default)]
    pub asset: Option<OneOrMany<MasonAsset>>,
    #[serde(default)]
    pub download: Option<OneOrMany<MasonDownload>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        match self {
            Self::One(value) => std::slice::from_ref(value),
            Self::Many(values) => values,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonAsset {
    #[serde(default)]
    pub target: Option<OneOrMany<String>>,
    pub file: OneOrMany<String>,
    #[serde(default)]
    pub bin: Option<MasonAssetBin>,
    #[serde(default)]
    pub ext: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MasonAssetBin {
    One(String),
    Many(BTreeMap<String, String>),
}

impl MasonAssetBin {
    #[must_use]
    pub fn as_single(&self) -> Option<&str> {
        match self {
            Self::One(value) => Some(value),
            Self::Many(_) => None,
        }
    }

    #[must_use]
    pub fn as_map(&self) -> Option<&BTreeMap<String, String>> {
        match self {
            Self::One(_) => None,
            Self::Many(values) => Some(values),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonDownload {
    #[serde(default)]
    pub target: Option<OneOrMany<String>>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    pub bin: Option<String>,
    pub config: Option<String>,
    #[serde(default)]
    pub man: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonNeovim {
    pub lspconfig: Option<String>,
}

fn source_id_version(source_id: &str) -> Result<&str, String> {
    source_id
        .strip_prefix("pkg:")
        .and_then(|value| value.rsplit_once('@').map(|(_, version)| version))
        .ok_or_else(|| format!("unsupported Mason package source {source_id}"))
}

fn version_matches_constraint(version: &str, constraint: &str) -> bool {
    let Some(upper_bound) = constraint_upper_bound(constraint) else {
        return false;
    };

    compare_versions(Some(version), Some(upper_bound)).is_le()
}

fn constraint_upper_bound(constraint: &str) -> Option<&str> {
    constraint
        .strip_prefix("semver:")
        .and_then(|value| value.strip_prefix("<="))
}

fn compare_versions(left: Option<&str>, right: Option<&str>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_version_strings(left, right),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn compare_version_strings(left: &str, right: &str) -> std::cmp::Ordering {
    let left = tokenize_version(left);
    let right = tokenize_version(right);
    let max_len = left.len().max(right.len());

    for index in 0..max_len {
        let ordering = compare_version_part(left.get(index), right.get(index));
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }

    std::cmp::Ordering::Equal
}

fn tokenize_version(version: &str) -> Vec<VersionPart> {
    let trimmed = version.trim_start_matches('v');
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = None;

    for ch in trimmed.chars() {
        if !ch.is_ascii_alphanumeric() {
            push_version_part(&mut parts, &mut current, &mut current_is_digit);
            continue;
        }

        let is_digit = ch.is_ascii_digit();
        if current_is_digit.is_some_and(|value| value != is_digit) {
            push_version_part(&mut parts, &mut current, &mut current_is_digit);
        }
        current_is_digit = Some(is_digit);
        current.push(ch);
    }

    push_version_part(&mut parts, &mut current, &mut current_is_digit);
    parts
}

fn push_version_part(
    parts: &mut Vec<VersionPart>,
    current: &mut String,
    current_is_digit: &mut Option<bool>,
) {
    if current.is_empty() {
        *current_is_digit = None;
        return;
    }

    if current_is_digit.unwrap_or(false) {
        parts.push(VersionPart::Number(
            current.parse::<u64>().unwrap_or(u64::MAX),
        ));
    } else {
        parts.push(VersionPart::Text(current.to_ascii_lowercase()));
    }

    current.clear();
    *current_is_digit = None;
}

#[derive(Debug)]
enum VersionPart {
    Number(u64),
    Text(String),
}

fn compare_version_part(
    left: Option<&VersionPart>,
    right: Option<&VersionPart>,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(VersionPart::Number(left)), Some(VersionPart::Number(right))) => left.cmp(right),
        (Some(VersionPart::Text(left)), Some(VersionPart::Text(right))) => left.cmp(right),
        (Some(VersionPart::Number(left)), Some(VersionPart::Text(_))) => {
            if *left == 0 {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }
        (Some(VersionPart::Text(_)), Some(VersionPart::Number(right))) => {
            if *right == 0 {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        }
        (Some(part), None) => compare_version_part(Some(part), Some(&VersionPart::Number(0))),
        (None, Some(part)) => compare_version_part(Some(&VersionPart::Number(0)), Some(part)),
        (None, None) => std::cmp::Ordering::Equal,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RegistryMetadata {
    release_tag: String,
    refreshed_at_epoch_seconds: u64,
    digest: Option<String>,
}

impl RegistryMetadata {
    fn is_fresh_at(&self, now_epoch_seconds: u64) -> bool {
        now_epoch_seconds.saturating_sub(self.refreshed_at_epoch_seconds)
            <= REGISTRY_FRESHNESS_THRESHOLD.as_secs()
    }
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

fn ensure_registry_cache(state: &RuntimeState) -> Result<(), String> {
    let registry_json_path = state.registry_json_path();
    let metadata_path = state.registry_metadata_path();
    let now_epoch_seconds = unix_timestamp_now()?;

    if let Some(metadata) = read_registry_metadata(&metadata_path)?
        && metadata.is_fresh_at(now_epoch_seconds)
        && registry_json_path.is_file()
    {
        return Ok(());
    }

    refresh_registry_cache(state, now_epoch_seconds)
}

fn refresh_registry_cache(state: &RuntimeState, now_epoch_seconds: u64) -> Result<(), String> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("failed to create HTTP client: {error}"))?;

    let release = fetch_latest_release(&client)?;
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == REGISTRY_ASSET_NAME)
        .ok_or_else(|| "Mason registry release does not include registry.json.zip".to_string())?;
    let metadata_path = state.registry_metadata_path();
    let registry_json_path = state.registry_json_path();

    if let Some(existing) = read_registry_metadata(&metadata_path)?
        && existing.release_tag == release.tag_name
        && registry_json_path.is_file()
    {
        let refreshed = RegistryMetadata {
            release_tag: existing.release_tag,
            refreshed_at_epoch_seconds: now_epoch_seconds,
            digest: existing.digest,
        };
        write_json_file(&metadata_path, &refreshed)?;
        return Ok(());
    }

    let archive_bytes = download_bytes(&client, &asset.browser_download_url)?;
    verify_sha256(&archive_bytes, asset.digest.as_deref())?;
    let registry_bytes = unpack_registry_json(&archive_bytes)?;

    write_bytes_file(&registry_json_path, &registry_bytes)?;
    write_json_file(
        &metadata_path,
        &RegistryMetadata {
            release_tag: release.tag_name,
            refreshed_at_epoch_seconds: now_epoch_seconds,
            digest: asset.digest,
        },
    )?;

    Ok(())
}

fn fetch_latest_release(client: &Client) -> Result<GithubRelease, String> {
    let response = client
        .get(GITHUB_API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|error| format!("failed to contact GitHub for Mason registry metadata: {error}"))?
        .error_for_status()
        .map_err(|error| format!("failed to fetch Mason registry metadata: {error}"))?;

    response
        .json::<GithubRelease>()
        .map_err(|error| format!("failed to parse Mason registry metadata: {error}"))
}

fn download_bytes(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .map_err(|error| format!("failed to download Mason registry archive: {error}"))?
        .error_for_status()
        .map_err(|error| format!("failed to download Mason registry archive: {error}"))?;
    let mut bytes = Vec::new();
    response
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read Mason registry archive: {error}"))?;
    Ok(bytes)
}

fn verify_sha256(bytes: &[u8], digest: Option<&str>) -> Result<(), String> {
    let Some(digest) = digest else {
        return Ok(());
    };
    let expected = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("unsupported Mason registry digest format: {digest}"))?;
    let actual = format!("{:x}", Sha256::digest(bytes));

    if actual == expected {
        Ok(())
    } else {
        Err("downloaded Mason registry archive failed integrity verification".to_string())
    }
}

fn unpack_registry_json(archive_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(archive_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| format!("failed to open Mason registry archive: {error}"))?;
    let mut file = archive
        .by_name("registry.json")
        .map_err(|error| format!("failed to read registry.json from Mason archive: {error}"))?;
    let mut registry_bytes = Vec::new();
    file.read_to_end(&mut registry_bytes)
        .map_err(|error| format!("failed to unpack Mason registry data: {error}"))?;
    Ok(registry_bytes)
}

fn read_registry_metadata(path: &Path) -> Result<Option<RegistryMetadata>, String> {
    match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents)
            .map(Some)
            .map_err(|error| format!("failed to parse {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

fn write_bytes_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!(
            "failed to determine parent directory for {}",
            path.display()
        ));
    };

    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let mut temp = NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "failed to create temporary file in {}: {error}",
            parent.display()
        )
    })?;
    temp.write_all(bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    temp.persist(path)
        .map_err(|error| format!("failed to persist {}: {error}", path.display()))?;
    Ok(())
}

fn write_json_file<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    write_bytes_file(path, &bytes)
}

fn unix_timestamp_now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("failed to read system clock: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        MasonAsset, MasonAssetBin, MasonNeovim, MasonPackage, MasonRegistry, MasonSource,
        MasonVersionOverride, OneOrMany, RegistryMetadata,
    };
    use crate::runtime_state::RuntimeState;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lsp-cli-mason-registry-test-{}-{}",
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn keeps_only_lsp_packages_with_lspconfig_mapping() {
        let registry = MasonRegistry::from_packages(vec![
            MasonPackage {
                name: "pyright".to_string(),
                categories: vec!["LSP".to_string()],
                source: MasonSource {
                    id: "pkg:npm/pyright@1.0.0".to_string(),
                    extra_packages: Vec::new(),
                    asset: None,
                    download: None,
                    version_overrides: Vec::new(),
                },
                bin: BTreeMap::from([(
                    "pyright-langserver".to_string(),
                    "npm:pyright-langserver".to_string(),
                )]),
                share: BTreeMap::new(),
                neovim: MasonNeovim {
                    lspconfig: Some("pyright".to_string()),
                },
            },
            MasonPackage {
                name: "stylua".to_string(),
                categories: vec!["Formatter".to_string()],
                source: MasonSource {
                    id: "pkg:github/john/stylua@1.0.0".to_string(),
                    extra_packages: Vec::new(),
                    asset: None,
                    download: None,
                    version_overrides: Vec::new(),
                },
                bin: BTreeMap::new(),
                share: BTreeMap::new(),
                neovim: MasonNeovim::default(),
            },
        ]);

        assert_eq!(
            registry
                .package_for_lspconfig("pyright")
                .expect("pyright should be indexed")
                .name,
            "pyright"
        );
        assert!(registry.package_for_lspconfig("stylua").is_none());
    }

    fn package(name: &str, lspconfig: Option<&str>, bin_name: &str) -> MasonPackage {
        MasonPackage {
            name: name.to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: format!("pkg:npm/{name}@1.0.0"),
                extra_packages: Vec::new(),
                asset: None,
                download: None,
                version_overrides: Vec::new(),
            },
            bin: BTreeMap::from([(bin_name.to_string(), format!("npm:{bin_name}"))]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: lspconfig.map(str::to_string),
            },
        }
    }

    #[test]
    fn falls_back_to_package_name_and_binary_name() {
        let registry = MasonRegistry::from_packages(vec![
            package("aiken", Some("aiken_lsp"), "aiken"),
            package(
                "ada-language-server",
                Some("ada_language_server"),
                "ada_language_server",
            ),
        ]);

        assert_eq!(
            registry
                .package_for_detected("aiken", "aiken", "aiken")
                .expect("package-name fallback should resolve")
                .name,
            "aiken"
        );
        assert_eq!(
            registry
                .package_for_detected("ada_ls", "ada_language_server", "ada_language_server")
                .expect("binary-name fallback should resolve")
                .name,
            "ada-language-server"
        );
    }

    #[test]
    fn applies_most_specific_matching_version_override() {
        let registry = MasonRegistry::from_packages(vec![MasonPackage {
            name: "angular-language-server".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:npm/@angular/language-server@17.3.2".to_string(),
                extra_packages: vec!["typescript@latest".to_string()],
                asset: None,
                download: None,
                version_overrides: vec![
                    MasonVersionOverride {
                        constraint: "semver:<=19.2.4".to_string(),
                        id: "pkg:npm/@angular/language-server@19.2.4".to_string(),
                        extra_packages: Some(vec!["typescript@5.8.3".to_string()]),
                        asset: None,
                        download: None,
                    },
                    MasonVersionOverride {
                        constraint: "semver:<=17.3.2".to_string(),
                        id: "pkg:npm/@angular/language-server@17.3.2".to_string(),
                        extra_packages: Some(vec!["typescript@5.3.2".to_string()]),
                        asset: None,
                        download: None,
                    },
                ],
            },
            bin: BTreeMap::from([("ngserver".to_string(), "npm:ngserver".to_string())]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("angularls".to_string()),
            },
        }]);

        let package = registry
            .package_for_lspconfig("angularls")
            .expect("angular package should be indexed");

        assert_eq!(package.source.id, "pkg:npm/@angular/language-server@17.3.2");
        assert_eq!(package.source.extra_packages, vec!["typescript@5.3.2"]);
    }

    #[test]
    fn applies_version_override_asset_payload() {
        let registry = MasonRegistry::from_packages(vec![MasonPackage {
            name: "rubyfmt".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:github/fables-tales/rubyfmt@v0.8.1".to_string(),
                extra_packages: Vec::new(),
                asset: Some(OneOrMany::One(MasonAsset {
                    target: Some(OneOrMany::One("linux_x64_gnu".to_string())),
                    file: OneOrMany::One("rubyfmt-latest.tar.gz".to_string()),
                    bin: Some(MasonAssetBin::One("rubyfmt".to_string())),
                    ext: None,
                })),
                download: None,
                version_overrides: vec![MasonVersionOverride {
                    constraint: "semver:<=v0.8.1".to_string(),
                    id: "pkg:github/fables-tales/rubyfmt@v0.8.1".to_string(),
                    extra_packages: None,
                    asset: Some(OneOrMany::One(MasonAsset {
                        target: Some(OneOrMany::One("linux_x64_gnu".to_string())),
                        file: OneOrMany::One("rubyfmt-v0.8.1-Linux.tar.gz".to_string()),
                        bin: Some(MasonAssetBin::One(
                            "tmp/releases/{{version}}-Linux/rubyfmt".to_string(),
                        )),
                        ext: None,
                    })),
                    download: None,
                }],
            },
            bin: BTreeMap::from([("rubyfmt".to_string(), "{{source.asset.bin}}".to_string())]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("rubyfmt".to_string()),
            },
        }]);

        let package = registry
            .package_for_lspconfig("rubyfmt")
            .expect("rubyfmt package should be indexed");

        assert_eq!(
            package.source.assets()[0].file.as_slice()[0],
            "rubyfmt-v0.8.1-Linux.tar.gz"
        );
    }

    #[test]
    fn parses_object_valued_asset_bin_mapping() {
        let package = serde_json::from_value::<MasonPackage>(serde_json::json!({
            "name": "kcl",
            "categories": ["LSP"],
            "source": {
                "id": "pkg:github/kcl-lang/kcl@v0.11.2",
                "asset": [{
                    "target": "linux_x64_gnu",
                    "file": "kclvm-v0.11.2-linux-amd64.tar.gz",
                    "bin": {
                        "kcl": "exec:kclvm/bin/kclvm_cli",
                        "kcl_language_server": "exec:kclvm/bin/kcl-language-server"
                    }
                }]
            },
            "bin": {
                "kcl-language-server": "{{source.asset.bin.kcl_language_server}}"
            },
            "neovim": {
                "lspconfig": "kcl"
            }
        }))
        .expect("package should parse");

        let asset_bin = package.source.assets()[0]
            .bin
            .as_ref()
            .and_then(MasonAssetBin::as_map)
            .expect("object-valued bin should parse as map");

        assert_eq!(
            asset_bin.get("kcl_language_server"),
            Some(&"exec:kclvm/bin/kcl-language-server".to_string())
        );
    }

    #[test]
    fn load_cached_returns_none_when_registry_is_missing() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));

        assert!(MasonRegistry::load_cached(&state).is_none());
    }

    #[test]
    fn load_cached_returns_none_for_corrupted_registry() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));
        fs::create_dir_all(state.registry_dir()).expect("registry dir should be created");
        fs::write(state.registry_json_path(), b"{not json]")
            .expect("corrupted registry should be written");

        assert!(MasonRegistry::load_cached(&state).is_none());
    }

    #[test]
    fn registry_metadata_freshness_respects_threshold() {
        let metadata = RegistryMetadata {
            release_tag: "2026-01-01".to_string(),
            refreshed_at_epoch_seconds: 10,
            digest: None,
        };

        assert!(metadata.is_fresh_at(10 + 30 * 24 * 60 * 60));
        assert!(!metadata.is_fresh_at(11 + 30 * 24 * 60 * 60));
    }
}
