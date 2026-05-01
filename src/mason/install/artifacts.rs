use crate::mason::install::join_relative_path;
use crate::mason::platform::MasonPlatform;
use crate::mason::registry::{MasonAsset, MasonAssetBin, MasonDownload, MasonPackage, OneOrMany};
use crate::mason::template::TemplateContext;
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tar::Archive;
use zip::ZipArchive;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

pub(super) struct RenderedAssetData {
    bin: Option<String>,
    pub(super) file: String,
    ext: Option<String>,
    named_bins: BTreeMap<String, String>,
}

impl RenderedAssetData {
    pub(super) fn template_context<'a>(&'a self, version: &'a str) -> TemplateContext<'a> {
        TemplateContext {
            version,
            source_asset_bin: self.bin.as_deref(),
            source_asset_file: Some(&self.file),
            source_asset_ext: self.ext.as_deref(),
            source_download_bin: None,
            source_download_config: None,
            source_download_man: None,
            source_asset_named_bins: self.named_bins.clone(),
        }
    }
}

pub(super) struct RenderedDownloadData {
    bin: Option<String>,
    config: Option<String>,
    man: Option<String>,
}

impl RenderedDownloadData {
    pub(super) fn template_context<'a>(&'a self, version: &'a str) -> TemplateContext<'a> {
        TemplateContext {
            version,
            source_asset_bin: None,
            source_asset_file: None,
            source_asset_ext: None,
            source_download_bin: self.bin.as_deref(),
            source_download_config: self.config.as_deref(),
            source_download_man: self.man.as_deref(),
            source_asset_named_bins: BTreeMap::new(),
        }
    }
}

pub(super) fn render_asset_data(
    asset: &MasonAsset,
    version: &str,
    program: &str,
    package_name: &str,
) -> Result<RenderedAssetData, String> {
    let base_context = TemplateContext {
        version,
        source_asset_bin: None,
        source_asset_file: None,
        source_asset_ext: None,
        source_download_bin: None,
        source_download_config: None,
        source_download_man: None,
        source_asset_named_bins: BTreeMap::new(),
    };
    let named_bins = asset
        .bin
        .as_ref()
        .and_then(MasonAssetBin::as_map)
        .map(|bins| {
            bins.iter()
                .map(|(name, value)| (name.clone(), base_context.render(value)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let bin = asset
        .bin
        .as_ref()
        .and_then(MasonAssetBin::as_single)
        .map(|value| base_context.render(value))
        .or_else(|| named_bins.get(program).cloned());
    let ext = asset.ext.as_deref().map(|value| base_context.render(value));
    let file_context = TemplateContext {
        version,
        source_asset_bin: bin.as_deref(),
        source_asset_file: None,
        source_asset_ext: ext.as_deref(),
        source_download_bin: None,
        source_download_config: None,
        source_download_man: None,
        source_asset_named_bins: named_bins.clone(),
    };
    let file = file_context.render(asset.file.as_slice().first().ok_or_else(|| {
        format!("cannot install {package_name} because its GitHub asset file list is empty")
    })?);

    Ok(RenderedAssetData {
        bin,
        file,
        ext,
        named_bins,
    })
}

pub(super) fn render_download_data(
    download: &MasonDownload,
    version: &str,
) -> RenderedDownloadData {
    let base_context = TemplateContext {
        version,
        source_asset_bin: None,
        source_asset_file: None,
        source_asset_ext: None,
        source_download_bin: None,
        source_download_config: None,
        source_download_man: None,
        source_asset_named_bins: BTreeMap::new(),
    };
    let bin = download
        .bin
        .as_deref()
        .map(|value| base_context.render(value));
    let field_context = TemplateContext {
        version,
        source_asset_bin: None,
        source_asset_file: None,
        source_asset_ext: None,
        source_download_bin: bin.as_deref(),
        source_download_config: None,
        source_download_man: None,
        source_asset_named_bins: BTreeMap::new(),
    };
    let config = download
        .config
        .as_deref()
        .map(|value| field_context.render(value));
    let man = download
        .man
        .as_deref()
        .map(|value| field_context.render(value));

    RenderedDownloadData { bin, config, man }
}

pub(super) fn select_asset<'a>(
    package: &'a MasonPackage,
    platform: &MasonPlatform,
) -> Result<&'a MasonAsset, String> {
    package
        .source
        .assets()
        .iter()
        .find(|asset| matches_platform(asset.target.as_ref(), platform))
        .ok_or_else(|| format!("cannot install {} on this platform", package.name))
}

pub(super) fn select_download<'a>(
    package: &'a MasonPackage,
    platform: &MasonPlatform,
) -> Result<&'a MasonDownload, String> {
    package
        .source
        .downloads()
        .iter()
        .find(|download| matches_platform(download.target.as_ref(), platform))
        .ok_or_else(|| format!("cannot install {} on this platform", package.name))
}

fn matches_platform(targets: Option<&OneOrMany<String>>, platform: &MasonPlatform) -> bool {
    targets.is_none_or(|targets| {
        targets
            .as_slice()
            .iter()
            .any(|target| platform.matches(target))
    })
}

pub(super) fn ensure_command_success(
    output: &std::process::Output,
    package: &MasonPackage,
    command_name: &str,
) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("command failed");
    Err(format!(
        "cannot install {} because {} failed: {detail}",
        package.name, command_name
    ))
}

pub(super) fn http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("failed to create HTTP client: {error}"))
}

pub(super) fn download_bytes(
    client: &Client,
    url: &str,
    package: &MasonPackage,
) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .map_err(|error| format!("failed to download {}: {error}", package.name))?
        .error_for_status()
        .map_err(|error| format!("failed to download {}: {error}", package.name))?;
    let mut bytes = Vec::new();
    response
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read download for {}: {error}", package.name))?;
    Ok(bytes)
}

pub(super) fn install_downloaded_artifact(
    root: &Path,
    relative_name: &str,
    bytes: &[u8],
) -> Result<(), String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("failed to create {}: {error}", root.display()))?;
    let relative_name_lower = relative_name.to_ascii_lowercase();
    let extension = Path::new(relative_name)
        .extension()
        .and_then(|value| value.to_str());

    if relative_name_lower.ends_with(".tar.gz") {
        extract_tar_gz(root, bytes)
    } else if extension.is_some_and(|value| value.eq_ignore_ascii_case("zip")) {
        extract_zip(root, bytes)
    } else if extension.is_some_and(|value| value.eq_ignore_ascii_case("gz")) {
        let target = join_relative_path(root, &relative_name[..relative_name.len() - 3])?;
        write_gzip_file(&target, bytes)
    } else {
        let path = join_relative_path(root, relative_name)?;
        write_file(&path, bytes)
    }
}

fn extract_tar_gz(root: &Path, bytes: &[u8]) -> Result<(), String> {
    let reader = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(reader);
    for entry in archive.entries().map_err(|error| {
        format!(
            "failed to open downloaded tar archive in {}: {error}",
            root.display()
        )
    })? {
        let mut entry = entry.map_err(|error| {
            format!(
                "failed to read downloaded tar archive in {}: {error}",
                root.display()
            )
        })?;
        let entry_path = entry.path().map_err(|error| {
            format!(
                "failed to read tar entry path in {}: {error}",
                root.display()
            )
        })?;
        let output_path = join_relative_path(root, &entry_path.to_string_lossy())?;
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("failed to create {}: {error}", output_path.display()))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        entry
            .unpack(&output_path)
            .map_err(|error| format!("failed to extract {}: {error}", output_path.display()))?;
    }
    Ok(())
}

fn extract_zip(root: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        format!(
            "failed to open downloaded zip archive in {}: {error}",
            root.display()
        )
    })?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            format!(
                "failed to read downloaded zip archive in {}: {error}",
                root.display()
            )
        })?;
        let Some(name) = file.enclosed_name() else {
            return Err(format!(
                "downloaded zip archive contains unsafe paths for {}",
                root.display()
            ));
        };
        let output_path = root.join(name);
        if file.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("failed to create {}: {error}", output_path.display()))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        let mut output = fs::File::create(&output_path)
            .map_err(|error| format!("failed to create {}: {error}", output_path.display()))?;
        std::io::copy(&mut file, &mut output)
            .map_err(|error| format!("failed to extract {}: {error}", output_path.display()))?;
        #[cfg(unix)]
        if let Some(mode) = file.unix_mode() {
            fs::set_permissions(&output_path, fs::Permissions::from_mode(mode)).map_err(
                |error| {
                    format!(
                        "failed to set permissions on {}: {error}",
                        output_path.display()
                    )
                },
            )?;
        }
    }
    Ok(())
}

fn write_gzip_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut decoder = GzDecoder::new(Cursor::new(bytes));
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|error| format!("failed to unpack {}: {error}", path.display()))?;
    write_file(path, &output)
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!(
            "failed to determine parent directory for {}",
            path.display()
        ));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(super) fn parse_archive_file_spec(file: &str) -> (&str, Option<&str>) {
    match file.split_once(':') {
        Some((archive, directory)) => (archive, Some(directory.trim_end_matches('/'))),
        None => (file, None),
    }
}
