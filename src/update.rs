use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::Deserialize;
use tar::Archive;
use zip::ZipArchive;

use crate::config::{CliConfig, load_cli_config, load_config_store};
use crate::error::{Error, Result};
use crate::runtime_state::{RuntimeState, default_runtime_state_root};

const DATA_REPOSITORY: &str = "segoon/lsp-cli-data";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

pub(crate) fn load_cli_defaults_for_update() -> Result<CliConfig> {
    let roots = crate::config::CliConfigRoots::default();
    load_cli_config(&roots.global, roots.user.as_deref())
        .map_err(|error| error.with_prefix("failed to load lsp-cli defaults"))
}

pub(crate) fn ensure_data_available() -> Result<()> {
    if crate::config::default_config_root().is_ok() {
        return Ok(());
    }

    let cli = load_cli_defaults_for_update()?;
    run_update_with_version(cli.download_version.as_deref().unwrap_or("latest"))?;

    crate::config::default_config_root().map(|_| ())
}

pub(crate) fn run_update_with_cli(cli: &CliConfig) -> Result<String> {
    let version = run_update_with_version(cli.download_version.as_deref().unwrap_or("latest"))?;
    Ok(format!("updated lsp-cli data to {version}"))
}

fn run_update_with_version(version: &str) -> Result<String> {
    let state = RuntimeState::new(default_runtime_state_root()?);
    state.ensure_dirs()?;

    let client = http_client()?;
    let release = fetch_release(&client, version)?;
    let archive = download_bytes(&client, &release.archive_url)?;
    install_downloaded_data(&state, &archive)?;
    Ok(release.version)
}

fn install_downloaded_data(state: &RuntimeState, archive: &[u8]) -> Result<()> {
    let root = state.root();
    let temp_dir = tempfile::Builder::new()
        .prefix("lsp-cli-data-")
        .tempdir_in(root)
        .map_err(|error| {
            Error::unexpected(format!(
                "failed to create temporary directory in {}: {error}",
                root.display()
            ))
        })?;
    extract_archive(temp_dir.path(), archive)?;
    let extracted_root = locate_data_root(temp_dir.path())?;

    // Validate every config file before replacing the live data tree.
    load_config_store(&extracted_root)?;
    let _ = load_cli_config(&extracted_root, None)?;

    let final_root = state.data_dir();
    let replacement_root = temp_dir.path().join("validated-data");
    if replacement_root.exists() {
        fs::remove_dir_all(&replacement_root)
            .map_err(|error| Error::unexpected(format!("failed to remove {}: {error}", replacement_root.display())))?;
    }
    fs::rename(&extracted_root, &replacement_root).map_err(|error| {
        Error::unexpected(format!(
            "failed to prepare downloaded data in {}: {error}",
            replacement_root.display()
        ))
    })?;
    if final_root.exists() {
        fs::remove_dir_all(&final_root)
            .map_err(|error| Error::unexpected(format!("failed to remove {}: {error}", final_root.display())))?;
    }
    fs::rename(&replacement_root, &final_root)
        .map_err(|error| Error::unexpected(format!("failed to install {}: {error}", final_root.display())))?;
    Ok(())
}

fn locate_data_root(root: &Path) -> Result<PathBuf> {
    if has_config_dirs(root) {
        return Ok(root.to_path_buf());
    }

    let entries = fs::read_dir(root)
        .map_err(|error| Error::unexpected(format!("failed to read {}: {error}", root.display())))?;
    for entry in entries {
        let entry = entry.map_err(|error| Error::unexpected(format!("failed to read {}: {error}", root.display())))?;
        let path = entry.path();
        if path.is_dir() && has_config_dirs(&path) {
            return Ok(path);
        }
    }

    Err(Error::config_format(
        "downloaded lsp-cli-data archive does not contain filetypes/ and lsp/ directories"
            .to_string(),
    ))
}

fn has_config_dirs(root: &Path) -> bool {
    root.join("filetypes").is_dir() && root.join("lsp").is_dir()
}

fn extract_archive(root: &Path, bytes: &[u8]) -> Result<()> {
    if is_zip(bytes) {
        extract_zip(root, bytes)
    } else {
        extract_tar_gz(root, bytes)
    }
}

fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

fn extract_tar_gz(root: &Path, bytes: &[u8]) -> Result<()> {
    let reader = GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = Archive::new(reader);
    for entry in archive.entries().map_err(|error| {
        Error::network(format!(
            "failed to open downloaded tar archive in {}: {error}",
            root.display()
        ))
    })? {
        let mut entry = entry.map_err(|error| {
            Error::network(format!(
                "failed to read downloaded tar archive in {}: {error}",
                root.display()
            ))
        })?;
        let entry_path = entry.path().map_err(|error| {
            Error::network(format!(
                "failed to read tar entry path in {}: {error}",
                root.display()
            ))
        })?;
        let output_path = root.join(entry_path.as_ref());
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| Error::unexpected(format!("failed to create {}: {error}", output_path.display())))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| Error::unexpected(format!("failed to create {}: {error}", parent.display())))?;
        }
        entry
            .unpack(&output_path)
            .map_err(|error| Error::network(format!("failed to extract {}: {error}", output_path.display())))?;
    }
    Ok(())
}

fn extract_zip(root: &Path, bytes: &[u8]) -> Result<()> {
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| {
        Error::network(format!(
            "failed to open downloaded zip archive in {}: {error}",
            root.display()
        ))
    })?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            Error::network(format!(
                "failed to read downloaded zip archive in {}: {error}",
                root.display()
            ))
        })?;
        let Some(name) = file.enclosed_name() else {
            return Err(Error::config_format(format!(
                "downloaded zip archive contains unsafe paths for {}",
                root.display()
            )));
        };
        let output_path = root.join(name);
        if file.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| Error::unexpected(format!("failed to create {}: {error}", output_path.display())))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| Error::unexpected(format!("failed to create {}: {error}", parent.display())))?;
        }
        let mut output = fs::File::create(&output_path)
            .map_err(|error| Error::unexpected(format!("failed to create {}: {error}", output_path.display())))?;
        std::io::copy(&mut file, &mut output)
            .map_err(|error| Error::network(format!("failed to extract {}: {error}", output_path.display())))?;
    }
    Ok(())
}

fn http_client() -> Result<Client> {
    Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| Error::network(format!("failed to create HTTP client: {error}")))
}

fn download_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    let mut response = client
        .get(url)
        .send()
        .map_err(|error| Error::network(format!("failed to download lsp-cli-data: {error}")))?
        .error_for_status()
        .map_err(|error| Error::network(format!("failed to download lsp-cli-data: {error}")))?;
    let mut bytes = Vec::new();
    response
        .read_to_end(&mut bytes)
        .map_err(|error| Error::network(format!("failed to read lsp-cli-data download: {error}")))?;
    Ok(bytes)
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    tarball_url: Option<String>,
    zipball_url: Option<String>,
}

struct ReleaseDownload {
    version: String,
    archive_url: String,
}

fn fetch_release(client: &Client, version: &str) -> Result<ReleaseDownload> {
    let url = if version == "latest" {
        format!("https://api.github.com/repos/{DATA_REPOSITORY}/releases/latest")
    } else {
        format!("https://api.github.com/repos/{DATA_REPOSITORY}/releases/tags/{version}")
    };
    let release: GithubRelease = client
        .get(url)
        .send()
        .map_err(|error| Error::network(format!("failed to query lsp-cli-data releases: {error}")))?
        .error_for_status()
        .map_err(|error| Error::network(format!("failed to query lsp-cli-data releases: {error}")))?
        .json()
        .map_err(|error| Error::network(format!("failed to parse lsp-cli-data release metadata: {error}")))?;
    let archive_url = release.tarball_url.or(release.zipball_url).ok_or_else(|| {
        Error::network("lsp-cli-data release does not provide a downloadable archive")
    })?;
    Ok(ReleaseDownload {
        version: release.tag_name,
        archive_url,
    })
}

#[cfg(test)]
mod tests;
