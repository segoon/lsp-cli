use crate::error::{Error, Result};
use crate::mason::link::{
    finalize_install, is_resolved_program_runnable, join_relative_path, resolve_program,
};
use crate::mason::platform::MasonPlatform;
use crate::mason::registry::MasonPackage;
use crate::mason::source::{SourceId, parse_source_id};
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use std::fs;
use std::process::Command;

#[cfg(all(test, unix))]
use std::os::unix::fs::PermissionsExt;

mod artifacts;

#[cfg(test)]
mod tests;

use artifacts::{
    download_bytes, ensure_command_success, http_client, install_downloaded_artifact,
    parse_archive_file_spec, render_asset_data, render_download_data, select_asset,
    select_download,
};

pub(crate) fn resolve_cached_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
) -> Result<Option<std::path::PathBuf>> {
    match parse_source_id(&package.source.id)? {
        SourceId::Npm { .. }
        | SourceId::Pypi { .. }
        | SourceId::Cargo { .. }
        | SourceId::Golang { .. } => {
            let resolved_program =
                resolve_program(package, program, state, &TemplateContext::empty())?;
            Ok(is_resolved_program_runnable(&resolved_program)
                .then(|| resolved_program.executable_path().to_path_buf()))
        }
        SourceId::Github { version, .. } => {
            resolve_cached_github_program(state, package, program, &version)
        }
        SourceId::Generic { version, .. } => {
            resolve_cached_generic_program(state, package, program, &version)
        }
        SourceId::Unsupported { .. } => Ok(None),
    }
}

pub(crate) fn resolve_or_install_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
) -> Result<std::path::PathBuf> {
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
        SourceId::Unsupported { kind } => Err(Error::unexpected(format!(
            "cannot install {} automatically yet because its Mason package uses unsupported backend `{kind}`",
            package.name
        ))),
    }
}

fn install_npm_package(
    state: &RuntimeState,
    package: &MasonPackage,
    package_name: &str,
    version: &str,
    program: &str,
) -> Result<std::path::PathBuf> {
    let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }
    require_command("npm", package, program)?;

    state.ensure_dirs()?;
    let install_dir = state.package_dir(&package.name);
    fs::create_dir_all(&install_dir).map_err(|error| {
        Error::unexpected(format!(
            "failed to create {}: {error}",
            install_dir.display()
        ))
    })?;

    #[cfg(test)]
    if fake_npm_install(&install_dir, program)? {
        return finalize_install(
            state,
            package,
            program,
            &resolved_program,
            &TemplateContext::empty(),
            "npm did not produce a runnable",
        );
    }

    let install_spec = format!("{package_name}@{version}");
    let output = Command::new("npm")
        .arg("install")
        .arg("--no-package-lock")
        .arg("--prefix")
        .arg(&install_dir)
        .arg(&install_spec)
        .args(&package.source.extra_packages)
        .output()
        .map_err(|error| {
            Error::unexpected(format!(
                "cannot install {} because npm could not start: {error}",
                package.name
            ))
        })?;
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

#[cfg(test)]
use crate::env_vars;

#[cfg(test)]
fn fake_npm_install(install_dir: &std::path::Path, program: &str) -> Result<bool> {
    let Some(fake_program) = env_vars::fake_npm_program() else {
        return Ok(false);
    };
    if fake_program != program {
        return Ok(false);
    }

    let executable = install_dir.join("node_modules/.bin").join(program);
    let parent = executable.parent().ok_or_else(|| {
        Error::unexpected(format!(
            "failed to create fake npm output {}: no parent directory",
            executable.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        Error::unexpected(format!(
            "failed to create fake npm output {}: {error}",
            executable.display()
        ))
    })?;
    fs::write(&executable, b"stub\n").map_err(|error| {
        Error::unexpected(format!(
            "failed to write fake npm output {}: {error}",
            executable.display()
        ))
    })?;

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&executable)
            .map_err(|error| {
                Error::unexpected(format!(
                    "failed to inspect fake npm output {}: {error}",
                    executable.display()
                ))
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).map_err(|error| {
            Error::unexpected(format!(
                "failed to mark fake npm output {} executable: {error}",
                executable.display()
            ))
        })?;
    }

    Ok(true)
}

fn install_pypi_package(
    state: &RuntimeState,
    package: &MasonPackage,
    package_name: &str,
    version: &str,
    extras: &[String],
    program: &str,
) -> Result<std::path::PathBuf> {
    let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }
    require_command("python3", package, program)?;

    state.ensure_dirs()?;
    let install_dir = state.package_dir(&package.name);
    fs::create_dir_all(&install_dir).map_err(|error| {
        Error::unexpected(format!(
            "failed to create {}: {error}",
            install_dir.display()
        ))
    })?;

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
            Error::unexpected(format!(
                "cannot install {} because python3 -m pip could not start: {error}",
                package.name
            ))
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
) -> Result<std::path::PathBuf> {
    let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }
    require_command("cargo", package, program)?;

    state.ensure_dirs()?;
    let install_dir = state.package_dir(&package.name);
    fs::create_dir_all(&install_dir).map_err(|error| {
        Error::unexpected(format!(
            "failed to create {}: {error}",
            install_dir.display()
        ))
    })?;

    let output = Command::new("cargo")
        .arg("install")
        .arg("--root")
        .arg(&install_dir)
        .arg("--version")
        .arg(version)
        .arg(package_name)
        .output()
        .map_err(|error| {
            Error::unexpected(format!(
                "cannot install {} because cargo could not start: {error}",
                package.name
            ))
        })?;
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
) -> Result<std::path::PathBuf> {
    let resolved_program = resolve_program(package, program, state, &TemplateContext::empty())?;
    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }
    require_command("go", package, program)?;

    state.ensure_dirs()?;
    let bin_dir = resolved_program.executable_path().parent().ok_or_else(|| {
        Error::unexpected(format!(
            "failed to determine installation directory for {}",
            package.name
        ))
    })?;
    fs::create_dir_all(bin_dir).map_err(|error| {
        Error::unexpected(format!("failed to create {}: {error}", bin_dir.display()))
    })?;

    let output = Command::new("go")
        .arg("install")
        .arg(format!("{module_path}@{version}"))
        .env("GOBIN", bin_dir)
        .output()
        .map_err(|error| {
            Error::unexpected(format!(
                "cannot install {} because go could not start: {error}",
                package.name
            ))
        })?;
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
) -> Result<std::path::PathBuf> {
    state.ensure_dirs()?;

    let platform = MasonPlatform::detect()?;
    let asset = select_asset(package, &platform)?;
    let rendered_asset = render_asset_data(asset, version, program, &package.name)?;
    let context = rendered_asset.template_context(version);
    let resolved_program = resolve_program(package, program, state, &context)?;

    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }

    let client = http_client()?;
    let (download_name, extract_subdir) = parse_archive_file_spec(&rendered_asset.file);
    let url =
        format!("https://github.com/{repository}/releases/download/{version}/{download_name}");
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
) -> Result<std::path::PathBuf> {
    state.ensure_dirs()?;

    let platform = MasonPlatform::detect()?;
    let download = select_download(package, &platform)?;
    let rendered_download = render_download_data(download, version);
    let context = rendered_download.template_context(version);
    let resolved_program = resolve_program(package, program, state, &context)?;

    if is_resolved_program_runnable(&resolved_program) {
        return Ok(resolved_program.executable_path().to_path_buf());
    }

    let client = http_client()?;
    let install_root = state.package_dir(&package.name);
    fs::create_dir_all(&install_root).map_err(|error| {
        Error::unexpected(format!(
            "failed to create {}: {error}",
            install_root.display()
        ))
    })?;

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
) -> Result<Option<std::path::PathBuf>> {
    let platform = MasonPlatform::detect()?;
    let asset = select_asset(package, &platform)?;
    let rendered_asset = render_asset_data(asset, version, program, &package.name)?;
    let context = rendered_asset.template_context(version);
    let resolved_program = resolve_program(package, program, state, &context)?;
    Ok(is_resolved_program_runnable(&resolved_program)
        .then(|| resolved_program.executable_path().to_path_buf()))
}

fn resolve_cached_generic_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
    version: &str,
) -> Result<Option<std::path::PathBuf>> {
    let platform = MasonPlatform::detect()?;
    let download = select_download(package, &platform)?;
    let rendered_download = render_download_data(download, version);
    let context = rendered_download.template_context(version);
    let resolved_program = resolve_program(package, program, state, &context)?;
    Ok(is_resolved_program_runnable(&resolved_program)
        .then(|| resolved_program.executable_path().to_path_buf()))
}

fn require_command(command: &str, package: &MasonPackage, program: &str) -> Result<()> {
    if crate::mason::link::is_command_runnable(command) {
        Ok(())
    } else {
        Err(Error::unexpected(format!(
            "cannot install {} because {} is not available in $PATH",
            suggestion_server_name(package, program),
            command
        )))
    }
}

fn suggestion_server_name(package: &MasonPackage, program: &str) -> String {
    if package.bin.contains_key(program) {
        program.to_string()
    } else {
        package.name.clone()
    }
}
