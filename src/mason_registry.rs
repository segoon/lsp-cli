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

const GITHUB_API_URL: &str = "https://api.github.com/repos/mason-org/mason-registry/releases/latest";
const REGISTRY_ASSET_NAME: &str = "registry.json.zip";
const REGISTRY_FRESHNESS_THRESHOLD: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MasonRegistry {
    packages_by_lspconfig: BTreeMap<String, MasonPackage>,
}

impl MasonRegistry {
    pub fn load(state: &RuntimeState) -> Result<Self, String> {
        state.ensure_dirs()?;

        let registry_json_path = state.registry_json_path();
        match ensure_registry_cache(state) {
            Ok(()) => Self::from_registry_json_path(&registry_json_path),
            Err(error) if registry_json_path.is_file() => {
                eprintln!(
                    "warning: failed to refresh Mason registry, using cached data: {error}"
                );
                Self::from_registry_json_path(&registry_json_path)
            }
            Err(error) => Err(error),
        }
    }

    pub fn package_for_lspconfig(&self, lspconfig: &str) -> Option<&MasonPackage> {
        self.packages_by_lspconfig.get(lspconfig)
    }

    fn from_registry_json_path(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let package_values = serde_json::from_str::<Vec<serde_json::Value>>(&contents)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        let mut packages = Vec::new();
        for value in package_values.into_iter().filter(is_lsp_package_value) {
            if let Ok(package) = serde_json::from_value::<MasonPackage>(value) {
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
        let mut packages_by_lspconfig = BTreeMap::new();

        for package in packages {
            if !package.is_lsp() {
                continue;
            }

            let Some(lspconfig) = package.neovim.lspconfig.clone() else {
                continue;
            };
            packages_by_lspconfig.insert(lspconfig, package);
        }

        Self {
            packages_by_lspconfig,
        }
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

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
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
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MasonSource {
    pub id: String,
    #[serde(default)]
    pub extra_packages: Vec<String>,
    #[serde(default)]
    pub asset: Option<OneOrMany<MasonAsset>>,
    #[serde(default)]
    pub download: Option<OneOrMany<MasonDownload>>,
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
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MasonAsset {
    #[serde(default)]
    pub target: Option<OneOrMany<String>>,
    pub file: OneOrMany<String>,
    pub bin: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MasonDownload {
    #[serde(default)]
    pub target: Option<OneOrMany<String>>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    pub bin: Option<String>,
    pub config: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct MasonNeovim {
    pub lspconfig: Option<String>,
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
        return Err(format!("failed to determine parent directory for {}", path.display()));
    };

    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let mut temp = NamedTempFile::new_in(parent)
        .map_err(|error| format!("failed to create temporary file in {}: {error}", parent.display()))?;
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
    use super::{MasonPackage, MasonRegistry, RegistryMetadata};
    use std::collections::BTreeMap;

    #[test]
    fn keeps_only_lsp_packages_with_lspconfig_mapping() {
        let registry = MasonRegistry::from_packages(vec![
            MasonPackage {
                name: "pyright".to_string(),
                categories: vec!["LSP".to_string()],
                source: super::MasonSource {
                    id: "pkg:npm/pyright@1.0.0".to_string(),
                    extra_packages: Vec::new(),
                    asset: None,
                    download: None,
                },
                bin: BTreeMap::from([(
                    "pyright-langserver".to_string(),
                    "npm:pyright-langserver".to_string(),
                )]),
                share: BTreeMap::new(),
                neovim: super::MasonNeovim {
                    lspconfig: Some("pyright".to_string()),
                },
            },
            MasonPackage {
                name: "stylua".to_string(),
                categories: vec!["Formatter".to_string()],
                source: super::MasonSource {
                    id: "pkg:github/john/stylua@1.0.0".to_string(),
                    extra_packages: Vec::new(),
                    asset: None,
                    download: None,
                },
                bin: BTreeMap::new(),
                share: BTreeMap::new(),
                neovim: super::MasonNeovim::default(),
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
