use crate::error::{Error, Result, error_fn};
use crate::hash::encode_hex;
use crate::mason::http::{
    download_bytes as http_download_bytes, read_json as http_read_json, send as http_send,
};
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

pub(super) fn ensure_registry_cache(state: &RuntimeState) -> Result<()> {
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

fn refresh_registry_cache(state: &RuntimeState, now_epoch_seconds: u64) -> Result<()> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(error_fn!(Error::network, "failed to create HTTP client"))?;

    let release = fetch_latest_release(&client)?;
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == REGISTRY_ASSET_NAME)
        .ok_or_else(|| {
            Error::network("Mason registry release does not include registry.json.zip")
        })?;
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
        "failed to download Mason registry archive",
        "failed to read Mason registry archive",
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

fn fetch_latest_release(client: &Client) -> Result<GithubRelease> {
    let response = http_send(
        client
            .get(GITHUB_API_URL)
            .header("Accept", "application/vnd.github+json"),
        "failed to contact GitHub for Mason registry metadata",
        "failed to fetch Mason registry metadata",
    )?;

    http_read_json(response, "failed to parse Mason registry metadata")
}

fn verify_sha256(bytes: &[u8], digest: Option<&str>) -> Result<()> {
    let Some(digest) = digest else {
        return Ok(());
    };
    let expected = digest.strip_prefix("sha256:").ok_or_else(|| {
        Error::network(format!(
            "unsupported Mason registry digest format: {digest}"
        ))
    })?;
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
