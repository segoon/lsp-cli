use crate::error::{Error, Result, error_fn};
use crate::hash::encode_hex;
use crate::http::retry::RetryPolicy;
use crate::http::{download_bytes as http_download_bytes, read_json as http_read_json};
use crate::runtime_state::RuntimeState;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;
use zip::ZipArchive;

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/mason-org/mason-registry/releases/latest";
const REGISTRY_ASSET_NAME: &str = "registry.json.zip";
const REGISTRY_FRESHNESS_THRESHOLD: Duration = Duration::from_hours(24 * 30);
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct RegistryMetadata {
    pub(super) release_tag: String,
    pub(super) refreshed_at_epoch_seconds: u64,
    pub(super) digest: Option<String>,
}

impl RegistryMetadata {
    pub(super) fn is_fresh_at(&self, now_epoch_seconds: u64) -> bool {
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

pub(super) fn ensure_registry_cache(
    state: &RuntimeState,
    github_token: Option<&str>,
) -> Result<()> {
    let registry_json_path = state.registry_json_path();
    let metadata_path = state.registry_metadata_path();
    let now_epoch_seconds = unix_timestamp_now()?;

    if let Some(metadata) = read_registry_metadata(&metadata_path)?
        && metadata.is_fresh_at(now_epoch_seconds)
        && registry_json_path.is_file()
    {
        return Ok(());
    }

    refresh_registry_cache(state, now_epoch_seconds, github_token)
}

fn refresh_registry_cache(
    state: &RuntimeState,
    now_epoch_seconds: u64,
    github_token: Option<&str>,
) -> Result<()> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(10))
        .retry(reqwest::retry::never())
        .build()
        .map_err(error_fn!(Error::network, "failed to create HTTP client"))?;

    let release = fetch_latest_release(&client, github_token)?;
    let Some(asset) = release
        .assets
        .into_iter()
        .find(|asset| asset.name == REGISTRY_ASSET_NAME)
    else {
        return Err(Error::network(
            "Mason registry release does not include registry.json.zip",
        ));
    };
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

    let archive_bytes = http_download_bytes(
        &client,
        &asset.browser_download_url,
        "failed to download Mason registry archive",
    )?;
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

fn fetch_latest_release(client: &Client, github_token: Option<&str>) -> Result<GithubRelease> {
    fetch_latest_release_from(
        client,
        GITHUB_API_URL,
        github_token,
        &RetryPolicy::default(),
        std::thread::sleep,
    )
}

fn fetch_latest_release_from(
    client: &Client,
    url: &str,
    github_token: Option<&str>,
    policy: &RetryPolicy,
    sleep: impl Fn(Duration) + 'static,
) -> Result<GithubRelease> {
    let mut request = client
        .get(url)
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = github_token.filter(|token| !token.is_empty()) {
        request = request.bearer_auth(token);
    }
    http_read_json(
        request,
        "failed to fetch Mason registry metadata",
        "failed to parse Mason registry metadata",
        policy,
        sleep,
    )
}

fn verify_sha256(bytes: &[u8], digest: Option<&str>) -> Result<()> {
    let Some(digest) = digest else {
        return Ok(());
    };
    let Some(expected) = digest.strip_prefix("sha256:") else {
        return Err(Error::network(format!(
            "unsupported Mason registry digest format: {digest}"
        )));
    };
    let actual = encode_hex(&Sha256::digest(bytes));

    if actual == expected {
        Ok(())
    } else {
        Err(Error::network(
            "downloaded Mason registry archive failed integrity verification",
        ))
    }
}

fn unpack_registry_json(archive_bytes: &[u8]) -> Result<Vec<u8>> {
    let cursor = Cursor::new(archive_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(error_fn!(
        Error::network,
        "failed to open Mason registry archive"
    ))?;
    let mut file = archive.by_name("registry.json").map_err(error_fn!(
        Error::network,
        "failed to read registry.json from Mason archive"
    ))?;
    let mut registry_bytes = Vec::new();
    file.read_to_end(&mut registry_bytes).map_err(error_fn!(
        Error::network,
        "failed to unpack Mason registry data"
    ))?;
    Ok(registry_bytes)
}

fn read_registry_metadata(path: &Path) -> Result<Option<RegistryMetadata>> {
    match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).map(Some).map_err(error_fn!(
            Error::unexpected,
            "failed to parse {}",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::unexpected(format!(
            "failed to read {}: {error}",
            path.display()
        ))),
    }
}

fn write_bytes_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Err(Error::unexpected(format!(
            "failed to determine parent directory for {}",
            path.display()
        )));
    };

    crate::fs::create_dir_all(parent)?;
    let mut temp = NamedTempFile::new_in(parent).map_err(error_fn!(
        Error::unexpected,
        "failed to create temporary file in {}",
        parent.display()
    ))?;
    temp.write_all(bytes).map_err(error_fn!(
        Error::unexpected,
        "failed to write {}",
        path.display()
    ))?;
    temp.persist(path).map_err(error_fn!(
        Error::unexpected,
        "failed to persist {}",
        path.display()
    ))?;
    Ok(())
}

fn write_json_file<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(error_fn!(
        Error::unexpected,
        "failed to serialize {}",
        path.display()
    ))?;
    write_bytes_file(path, &bytes)
}

fn unix_timestamp_now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(error_fn!(Error::unexpected, "failed to read system clock"))
}

#[cfg(test)]
#[path = "cache/tests.rs"]
mod tests;
