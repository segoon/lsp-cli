use crate::mason::link::{
    finalize_install, is_resolved_program_runnable, join_relative_path, resolve_program,
};
use crate::mason::platform::MasonPlatform;
use crate::mason::registry::{MasonAsset, MasonDownload, MasonPackage, OneOrMany};
use crate::mason::source::{SourceId, parse_source_id};
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use std::fs;
use std::io::{Cursor, Read};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tar::Archive;
use zip::ZipArchive;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

pub(crate) fn resolve_cached_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
) -> Result<Option<std::path::PathBuf>, String> {
    match parse_source_id(&package.source.id)? {
        SourceId::Npm { .. }
        | SourceId::Pypi { .. }
        | SourceId::Cargo { .. }
        | SourceId::Golang { .. } => {
            let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
            Ok(is_resolved_program_runnable(&resolved_program)
                .then(|| resolved_program.executable_path().to_path_buf()))
        }
        SourceId::Github { version, .. } => resolve_cached_github_program(state, package, program, &version),
        SourceId::Generic { version, .. } => resolve_cached_generic_program(state, package, program, &version),
        SourceId::Unsupported { .. } => Ok(None),
    }
}

pub(crate) fn resolve_or_install_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
) -> Result<std::path::PathBuf, String> {
    match parse_source_id(&package.source.id)? {
        SourceId::Npm {
            package_name,
            version,
        } => install_npm_package(state, package, &package_name, &version, program),
        SourceId::Pypi {
            package_name,
            version,
            extras,
        } => install_pypi_package(state, package, &package_name, &version, &extras, program),
        SourceId::Cargo {
            package_name,
            version,
        } => install_cargo_package(state, package, &package_name, &version, program),
        SourceId::Golang {
            module_path,
            version,
        } => install_golang_package(state, package, &module_path, &version, program),
        SourceId::Github {
            repository,
            version,
        } => install_github_package(state, package, &repository, &version, program),
        SourceId::Generic {
            package_name,
            version,
        } => install_generic_package(state, package, &package_name, &version, program),
        SourceId::Unsupported { kind } => Err(format!(
            "cannot install {} automatically yet because its Mason package uses unsupported backend `{kind}`",
            package.name
        )),
    }
}

fn install_npm_package(
    state: &RuntimeState,
    package: &MasonPackage,
    package_name: &str,
    version: &str,
    program: &str,
) -> Result<std::path::PathBuf, String> {
    let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }
    require_command("npm", package, program)?;

    state.ensure_dirs()?;
    let install_dir = state.package_dir(&package.name);
    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("failed to create {}: {error}", install_dir.display()))?;

    let install_spec = format!("{package_name}@{version}");
    let output = Command::new("npm")
        .arg("install")
        .arg("--no-package-lock")
        .arg("--prefix")
        .arg(&install_dir)
        .arg(&install_spec)
        .args(&package.source.extra_packages)
        .output()
        .map_err(|error| format!("cannot install {} because npm could not start: {error}", package.name))?;
    ensure_command_success(&output, package, "npm")?;

    finalize_install(
        state,
        package,
        program,
        &resolved_program,
        &TemplateContext::empty(),
        "npm did not produce a runnable",
    )
}

fn install_pypi_package(
    state: &RuntimeState,
    package: &MasonPackage,
    package_name: &str,
    version: &str,
    extras: &[String],
    program: &str,
) -> Result<std::path::PathBuf, String> {
    let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }
    require_command("python3", package, program)?;

    state.ensure_dirs()?;
    let install_dir = state.package_dir(&package.name);
    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("failed to create {}: {error}", install_dir.display()))?;

    let install_spec = if extras.is_empty() {
        format!("{package_name}=={version}")
    } else {
        format!("{package_name}[{}]=={version}", extras.join(","))
    };
    let output = Command::new("python3")
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--disable-pip-version-check")
        .arg("--prefix")
        .arg(&install_dir)
        .arg(&install_spec)
        .output()
        .map_err(|error| {
            format!(
                "cannot install {} because python3 -m pip could not start: {error}",
                package.name
            )
        })?;
    ensure_command_success(&output, package, "python3 -m pip")?;

    finalize_install(
        state,
        package,
        program,
        &resolved_program,
        &TemplateContext::empty(),
        "pip did not produce a runnable",
    )
}

fn install_cargo_package(
    state: &RuntimeState,
    package: &MasonPackage,
    package_name: &str,
    version: &str,
    program: &str,
) -> Result<std::path::PathBuf, String> {
    let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }
    require_command("cargo", package, program)?;

    state.ensure_dirs()?;
    let install_dir = state.package_dir(&package.name);
    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("failed to create {}: {error}", install_dir.display()))?;

    let output = Command::new("cargo")
        .arg("install")
        .arg("--root")
        .arg(&install_dir)
        .arg("--version")
        .arg(version)
        .arg(package_name)
        .output()
        .map_err(|error| format!("cannot install {} because cargo could not start: {error}", package.name))?;
    ensure_command_success(&output, package, "cargo")?;

    finalize_install(
        state,
        package,
        program,
        &resolved_program,
        &TemplateContext::empty(),
        "cargo did not produce a runnable",
    )
}

fn install_golang_package(
    state: &RuntimeState,
    package: &MasonPackage,
    module_path: &str,
    version: &str,
    program: &str,
) -> Result<std::path::PathBuf, String> {
    let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }
    require_command("go", package, program)?;

    state.ensure_dirs()?;
    let bin_dir = resolved_program.executable_path().parent().ok_or_else(|| {
        format!(
            "failed to determine installation directory for {}",
            package.name
        )
    })?;
    fs::create_dir_all(bin_dir)
        .map_err(|error| format!("failed to create {}: {error}", bin_dir.display()))?;

    let output = Command::new("go")
        .arg("install")
        .arg(format!("{module_path}@{version}"))
        .env("GOBIN", bin_dir)
        .output()
        .map_err(|error| format!("cannot install {} because go could not start: {error}", package.name))?;
    ensure_command_success(&output, package, "go")?;

    finalize_install(
        state,
        package,
        program,
        &resolved_program,
        &TemplateContext::empty(),
        "go did not produce a runnable",
    )
}

fn install_github_package(
    state: &RuntimeState,
    package: &MasonPackage,
    repository: &str,
    version: &str,
    program: &str,
) -> Result<std::path::PathBuf, String> {
    state.ensure_dirs()?;

    let platform = MasonPlatform::detect()?;
    let asset = select_asset(package, &platform)?;
    let asset_bin = asset.bin.as_deref().map(|value| {
        TemplateContext {
            version,
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: None,
            source_download_config: None,
        }
        .render(value)
    });
    let asset_file = TemplateContext {
        version,
        source_asset_bin: asset_bin.as_deref(),
        source_asset_file: None,
        source_download_bin: None,
        source_download_config: None,
    }
    .render(asset.file.as_slice().first().ok_or_else(|| {
        format!("cannot install {} because its GitHub asset file list is empty", package.name)
    })?);
    let context = TemplateContext {
        version,
        source_asset_bin: asset_bin.as_deref(),
        source_asset_file: Some(&asset_file),
        source_download_bin: None,
        source_download_config: None,
    };
    let resolved_program = resolve_program(package, program, state, &context)?;

    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }

    let client = http_client()?;
    let (download_name, extract_subdir) = parse_archive_file_spec(&asset_file);
    let url = format!(
        "https://github.com/{repository}/releases/download/{version}/{download_name}"
    );
    let archive_bytes = download_bytes(&client, &url, package)?;
    let install_dir = match extract_subdir {
        Some(path) => join_relative_path(&state.package_dir(&package.name), path)?,
        None => state.package_dir(&package.name),
    };
    install_downloaded_artifact(&install_dir, download_name, &archive_bytes)?;
    finalize_install(
        state,
        package,
        program,
        &resolved_program,
        &context,
        "the downloaded asset did not produce a runnable",
    )
}

fn install_generic_package(
    state: &RuntimeState,
    package: &MasonPackage,
    _package_name: &str,
    version: &str,
    program: &str,
) -> Result<std::path::PathBuf, String> {
    state.ensure_dirs()?;

    let platform = MasonPlatform::detect()?;
    let download = select_download(package, &platform)?;
    let download_bin = download.bin.as_deref().map(|value| {
        TemplateContext {
            version,
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: None,
            source_download_config: None,
        }
        .render(value)
    });
    let download_config = download.config.as_deref().map(|value| {
        TemplateContext {
            version,
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: download_bin.as_deref(),
            source_download_config: None,
        }
        .render(value)
    });
    let context = TemplateContext {
        version,
        source_asset_bin: None,
        source_asset_file: None,
        source_download_bin: download_bin.as_deref(),
        source_download_config: download_config.as_deref(),
    };
    let resolved_program = resolve_program(package, program, state, &context)?;

    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }

    let client = http_client()?;
    let install_root = state.package_dir(&package.name);
    fs::create_dir_all(&install_root)
        .map_err(|error| format!("failed to create {}: {error}", install_root.display()))?;

    for (relative_name, url_template) in &download.files {
        let relative_name = context.render(relative_name);
        let url = context.render(url_template);
        let bytes = download_bytes(&client, &url, package)?;
        install_downloaded_artifact(&install_root, &relative_name, &bytes)?;
    }

    finalize_install(
        state,
        package,
        program,
        &resolved_program,
        &context,
        "the downloaded files did not produce a runnable",
    )
}

fn resolve_cached_github_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
    version: &str,
) -> Result<Option<std::path::PathBuf>, String> {
    let platform = MasonPlatform::detect()?;
    let asset = select_asset(package, &platform)?;
    let asset_bin = asset.bin.as_deref().map(|value| {
        TemplateContext {
            version,
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: None,
            source_download_config: None,
        }
        .render(value)
    });
    let asset_file = TemplateContext {
        version,
        source_asset_bin: asset_bin.as_deref(),
        source_asset_file: None,
        source_download_bin: None,
        source_download_config: None,
    }
    .render(asset.file.as_slice().first().ok_or_else(|| {
        format!("cannot install {} because its GitHub asset file list is empty", package.name)
    })?);
    let context = TemplateContext {
        version,
        source_asset_bin: asset_bin.as_deref(),
        source_asset_file: Some(&asset_file),
        source_download_bin: None,
        source_download_config: None,
    };
    let resolved_program = resolve_program(package, program, state, &context)?;
    Ok(is_resolved_program_runnable(&resolved_program)
        .then(|| resolved_program.executable_path().to_path_buf()))
}

fn resolve_cached_generic_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
    version: &str,
) -> Result<Option<std::path::PathBuf>, String> {
    let platform = MasonPlatform::detect()?;
    let download = select_download(package, &platform)?;
    let download_bin = download.bin.as_deref().map(|value| {
        TemplateContext {
            version,
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: None,
            source_download_config: None,
        }
        .render(value)
    });
    let download_config = download.config.as_deref().map(|value| {
        TemplateContext {
            version,
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: download_bin.as_deref(),
            source_download_config: None,
        }
        .render(value)
    });
    let context = TemplateContext {
        version,
        source_asset_bin: None,
        source_asset_file: None,
        source_download_bin: download_bin.as_deref(),
        source_download_config: download_config.as_deref(),
    };
    let resolved_program = resolve_program(package, program, state, &context)?;
    Ok(is_resolved_program_runnable(&resolved_program)
        .then(|| resolved_program.executable_path().to_path_buf()))
}

fn select_asset<'a>(
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

fn select_download<'a>(
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
    targets.is_none_or(|targets| targets.as_slice().iter().any(|target| platform.matches(target)))
}

fn require_command(command: &str, package: &MasonPackage, program: &str) -> Result<(), String> {
    if crate::mason::link::is_command_runnable(command) {
        Ok(())
    } else {
        Err(format!(
            "cannot install {} because {} is not available in $PATH",
            suggestion_server_name(package, program),
            command
        ))
    }
}

fn suggestion_server_name(package: &MasonPackage, program: &str) -> String {
    if package.bin.contains_key(program) {
        program.to_string()
    } else {
        package.name.clone()
    }
}

fn ensure_command_success(
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

fn http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("failed to create HTTP client: {error}"))
}

fn download_bytes(client: &Client, url: &str, package: &MasonPackage) -> Result<Vec<u8>, String> {
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

fn install_downloaded_artifact(root: &Path, relative_name: &str, bytes: &[u8]) -> Result<(), String> {
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
        format!("failed to open downloaded tar archive in {}: {error}", root.display())
    })? {
        let mut entry = entry.map_err(|error| {
            format!("failed to read downloaded tar archive in {}: {error}", root.display())
        })?;
        let entry_path = entry.path().map_err(|error| {
            format!("failed to read tar entry path in {}: {error}", root.display())
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
        entry.unpack(&output_path)
            .map_err(|error| format!("failed to extract {}: {error}", output_path.display()))?;
    }
    Ok(())
}

fn extract_zip(root: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("failed to open downloaded zip archive in {}: {error}", root.display()))?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            format!("failed to read downloaded zip archive in {}: {error}", root.display())
        })?;
        let Some(name) = file.enclosed_name() else {
            return Err(format!("downloaded zip archive contains unsafe paths for {}", root.display()));
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
            fs::set_permissions(&output_path, fs::Permissions::from_mode(mode))
                .map_err(|error| format!("failed to set permissions on {}: {error}", output_path.display()))?;
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
        return Err(format!("failed to determine parent directory for {}", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn parse_archive_file_spec(file: &str) -> (&str, Option<&str>) {
    match file.split_once(':') {
        Some((archive, directory)) => (archive, Some(directory.trim_end_matches('/'))),
        None => (file, None),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_archive_file_spec;

    #[test]
    fn parses_archive_file_spec() {
        assert_eq!(
            parse_archive_file_spec("lua-language-server-3.18.2-linux-x64.tar.gz:libexec/"),
            ("lua-language-server-3.18.2-linux-x64.tar.gz", Some("libexec"))
        );
        assert_eq!(
            parse_archive_file_spec("clangd-linux-22.1.0.zip"),
            ("clangd-linux-22.1.0.zip", None)
        );
    }
}
