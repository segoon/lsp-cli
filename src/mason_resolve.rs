use crate::mason_platform::MasonPlatform;
use crate::mason_registry::{MasonAsset, MasonDownload, MasonPackage, MasonRegistry};
use crate::mason_template::TemplateContext;
use crate::runtime_state::{RuntimeState, default_runtime_state_root};
use crate::suggest::SuggestedLanguage;
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::{Cursor, Read};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use tar::Archive;
use zip::ZipArchive;

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

pub fn resolve_detect_suggestions(
    suggestions: &[SuggestedLanguage],
    download: bool,
) -> Result<Vec<SuggestedLanguage>, String> {
    if !download {
        return Ok(suggestions.to_vec());
    }

    let state = RuntimeState::new(default_runtime_state_root()?);
    let mut registry = None;
    let mut resolved = Vec::new();
    let mut errors = Vec::new();

    for suggestion in suggestions {
        match resolve_suggestion(suggestion, &state, &mut registry) {
            Ok(suggestion) => resolved.push(suggestion),
            Err(error) => errors.push(error),
        }
    }

    if !resolved.is_empty() {
        for error in errors {
            eprintln!("warning: {error}");
        }
        return Ok(resolved);
    }

    if errors.len() == 1 {
        Err(errors.remove(0))
    } else {
        Err(errors.join("\n"))
    }
}

fn resolve_suggestion(
    suggestion: &SuggestedLanguage,
    state: &RuntimeState,
    registry: &mut Option<MasonRegistry>,
) -> Result<SuggestedLanguage, String> {
    let Some(program) = suggestion.command.first() else {
        return Err(format!(
            "selected LSP server {} has an empty command",
            suggestion.server
        ));
    };

    if is_command_runnable(program) {
        return Ok(suggestion.clone());
    }

    if program.contains(std::path::MAIN_SEPARATOR) {
        return Err(format!(
            "configured LSP server executable `{program}` was not found"
        ));
    }

    let registry = registry.get_or_insert(MasonRegistry::load(state)?);
    let package = registry
        .package_for_lspconfig(&suggestion.config_id)
        .ok_or_else(|| {
            format!(
                "no Mason install recipe is available for detected server {}",
                suggestion.server
            )
        })?;
    let executable_path = resolve_or_install_program(state, package, program)?;

    Ok(rewrite_program(suggestion, &executable_path))
}

fn resolve_or_install_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
) -> Result<PathBuf, String> {
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
) -> Result<PathBuf, String> {
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

    finalize_install(state, package, program, &resolved_program, &TemplateContext::empty(), "npm did not produce a runnable")
}

fn install_pypi_package(
    state: &RuntimeState,
    package: &MasonPackage,
    package_name: &str,
    version: &str,
    extras: &[String],
    program: &str,
) -> Result<PathBuf, String> {
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

    finalize_install(state, package, program, &resolved_program, &TemplateContext::empty(), "pip did not produce a runnable")
}

fn install_cargo_package(
    state: &RuntimeState,
    package: &MasonPackage,
    package_name: &str,
    version: &str,
    program: &str,
) -> Result<PathBuf, String> {
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

    finalize_install(state, package, program, &resolved_program, &TemplateContext::empty(), "cargo did not produce a runnable")
}

fn install_golang_package(
    state: &RuntimeState,
    package: &MasonPackage,
    module_path: &str,
    version: &str,
    program: &str,
) -> Result<PathBuf, String> {
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

    finalize_install(state, package, program, &resolved_program, &TemplateContext::empty(), "go did not produce a runnable")
}

fn install_github_package(
    state: &RuntimeState,
    package: &MasonPackage,
    repository: &str,
    version: &str,
    program: &str,
) -> Result<PathBuf, String> {
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
    finalize_install(state, package, program, &resolved_program, &context, "the downloaded asset did not produce a runnable")
}

fn install_generic_package(
    state: &RuntimeState,
    package: &MasonPackage,
    _package_name: &str,
    version: &str,
    program: &str,
) -> Result<PathBuf, String> {
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
        let artifact_root = install_root.clone();
        install_downloaded_artifact(&artifact_root, &relative_name, &bytes)?;
    }

    finalize_install(state, package, program, &resolved_program, &context, "the downloaded files did not produce a runnable")
}

fn select_asset<'a>(package: &'a MasonPackage, platform: &MasonPlatform) -> Result<&'a MasonAsset, String> {
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

fn matches_platform(
    targets: Option<&crate::mason_registry::OneOrMany<String>>,
    platform: &MasonPlatform,
) -> bool {
    targets
        .is_none_or(|targets| targets.as_slice().iter().any(|target| platform.matches(target)))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ResolvedProgram {
    Direct(PathBuf),
    Wrapper(WrapperProgram),
}

impl ResolvedProgram {
    fn executable_path(&self) -> &Path {
        match self {
            Self::Direct(path) => path,
            Self::Wrapper(wrapper) => &wrapper.launcher_path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WrapperProgram {
    launcher_path: PathBuf,
    target_path: PathBuf,
    runtime: WrapperRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WrapperRuntime {
    Python,
    Node,
    Dotnet,
    Java,
}

impl WrapperRuntime {
    fn command_name(self) -> &'static str {
        match self {
            Self::Python => "python3",
            Self::Node => "node",
            Self::Dotnet => "dotnet",
            Self::Java => "java",
        }
    }

    fn script_line(self, target_path: &Path) -> String {
        let target = shell_quote(target_path);
        match self {
            Self::Python => format!("exec python3 {target} \"$@\""),
            Self::Node => format!("exec node {target} \"$@\""),
            Self::Dotnet => format!("exec dotnet {target} \"$@\""),
            Self::Java => format!("exec java -jar {target} \"$@\""),
        }
    }
}

fn resolve_program(
    package: &MasonPackage,
    program: &str,
    state: &RuntimeState,
    context: &TemplateContext<'_>,
) -> Result<ResolvedProgram, String> {
    let target = package.bin.get(program).ok_or_else(|| {
        format!(
            "cannot install {} because Mason does not expose executable {program}",
            package.name
        )
    })?;
    let rendered = context.render(target);

    if let Some(relative) = rendered.strip_prefix("npm:") {
        return Ok(ResolvedProgram::Direct(
            state
                .package_dir(&package.name)
                .join("node_modules")
                .join(".bin")
                .join(relative),
        ));
    }

    if let Some(relative) = rendered.strip_prefix("pypi:") {
        let root = state.package_dir(&package.name);
        let primary = root.join("bin").join(relative);
        if primary.exists() {
            return Ok(ResolvedProgram::Direct(primary));
        }
        let alternate = root.join("local").join("bin").join(relative);
        if alternate.exists() {
            return Ok(ResolvedProgram::Direct(alternate));
        }
        return Ok(ResolvedProgram::Direct(alternate));
    }

    if let Some(relative) = rendered.strip_prefix("cargo:") {
        return Ok(ResolvedProgram::Direct(
            state.package_dir(&package.name).join("bin").join(relative),
        ));
    }

    if let Some(relative) = rendered.strip_prefix("golang:") {
        return Ok(ResolvedProgram::Direct(
            state.package_dir(&package.name).join("bin").join(relative),
        ));
    }

    if let Some(relative) = rendered.strip_prefix("python:") {
        return Ok(ResolvedProgram::Wrapper(WrapperProgram {
            launcher_path: state.bin_dir().join(program),
            target_path: join_relative_path(&state.package_dir(&package.name), relative)?,
            runtime: WrapperRuntime::Python,
        }));
    }

    if let Some(relative) = rendered.strip_prefix("node:") {
        return Ok(ResolvedProgram::Wrapper(WrapperProgram {
            launcher_path: state.bin_dir().join(program),
            target_path: join_relative_path(&state.package_dir(&package.name), relative)?,
            runtime: WrapperRuntime::Node,
        }));
    }

    if let Some(relative) = rendered.strip_prefix("dotnet:") {
        return Ok(ResolvedProgram::Wrapper(WrapperProgram {
            launcher_path: state.bin_dir().join(program),
            target_path: join_relative_path(&state.package_dir(&package.name), relative)?,
            runtime: WrapperRuntime::Dotnet,
        }));
    }

    if let Some(relative) = rendered.strip_prefix("java-jar:") {
        return Ok(ResolvedProgram::Wrapper(WrapperProgram {
            launcher_path: state.bin_dir().join(program),
            target_path: join_relative_path(&state.package_dir(&package.name), relative)?,
            runtime: WrapperRuntime::Java,
        }));
    }

    let relative = rendered.strip_prefix("exec:").unwrap_or(&rendered);
    Ok(ResolvedProgram::Direct(join_relative_path(
        &state.package_dir(&package.name),
        relative,
    )?))
}

#[cfg(test)]
fn resolve_program_path(
    package: &MasonPackage,
    program: &str,
    state: &RuntimeState,
    context: &TemplateContext<'_>,
) -> Result<PathBuf, String> {
    Ok(resolve_program(package, program, state, context)?
        .executable_path()
        .to_path_buf())
}

fn is_resolved_program_runnable(program: &ResolvedProgram) -> bool {
    match program {
        ResolvedProgram::Direct(path) => is_command_runnable_path(path),
        ResolvedProgram::Wrapper(wrapper) => {
            is_command_runnable_path(&wrapper.launcher_path)
                && wrapper.target_path.is_file()
                && is_command_runnable(wrapper.runtime.command_name())
        }
    }
}

fn finalize_install(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
    resolved_program: &ResolvedProgram,
    context: &TemplateContext<'_>,
    failure_reason: &str,
) -> Result<PathBuf, String> {
    materialize_share(state, package, context)?;
    ensure_resolved_program(state, package, program, resolved_program)?;

    if !is_resolved_program_runnable(resolved_program) {
        return Err(format!(
            "cannot install {} because {failure_reason} {program} executable",
            package.name
        ));
    }

    write_receipt(state, package, resolved_program.executable_path())
}

fn ensure_resolved_program(
    _state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
    resolved_program: &ResolvedProgram,
) -> Result<(), String> {
    match resolved_program {
        ResolvedProgram::Direct(path) => ensure_executable(path),
        ResolvedProgram::Wrapper(wrapper) => {
            require_command(wrapper.runtime.command_name(), package, program)?;
            if !wrapper.target_path.is_file() {
                return Err(format!(
                    "cannot install {} because the wrapped launcher target {} was not created",
                    package.name,
                    wrapper.target_path.display()
                ));
            }
            write_wrapper_script(&wrapper.launcher_path, wrapper.runtime, &wrapper.target_path)?;
            ensure_executable(&wrapper.launcher_path)
        }
    }
}

fn materialize_share(
    state: &RuntimeState,
    package: &MasonPackage,
    context: &TemplateContext<'_>,
) -> Result<(), String> {
    for (target, source) in &package.share {
        let rendered_target = context.render(target);
        let rendered_source = context.render(source);
        let target_is_dir = rendered_target.ends_with('/');
        let source_is_dir = rendered_source.ends_with('/');
        let share_path = join_relative_path(&state.share_dir(), rendered_target.trim_end_matches('/'))?;
        let package_path = join_relative_path(
            &state.package_dir(&package.name),
            rendered_source.trim_end_matches('/'),
        )?;

        if target_is_dir || source_is_dir {
            copy_directory_contents(&package_path, &share_path)?;
        } else {
            copy_file(&package_path, &share_path)?;
        }
    }

    Ok(())
}

fn write_wrapper_script(
    launcher_path: &Path,
    runtime: WrapperRuntime,
    target_path: &Path,
) -> Result<(), String> {
    let contents = format!(
        "#!/bin/sh\n{}\n",
        runtime.script_line(target_path)
    );
    write_file(launcher_path, contents.as_bytes())
}

fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    let bytes = fs::read(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
    write_file(target, &bytes)
}

fn copy_directory_contents(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = fs::metadata(source)
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?;
    if !metadata.is_dir() {
        return Err(format!("expected directory at {}", source.display()));
    }

    fs::create_dir_all(target)
        .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = entry
            .metadata()
            .map_err(|error| format!("failed to inspect {}: {error}", source_path.display()))?;
        if metadata.is_dir() {
            copy_directory_contents(&source_path, &target_path)?;
        } else if metadata.is_file() {
            copy_file(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn shell_quote(path: &Path) -> String {
    let value = path.display().to_string().replace('\'', "'\"'\"'");
    format!("'{value}'")
}

fn require_command(command: &str, package: &MasonPackage, program: &str) -> Result<(), String> {
    if is_command_runnable(command) {
        Ok(())
    } else {
        Err(format!(
            "cannot install {} because {} is not available in $PATH",
            suggestion_server_name(package, program),
            command
        ))
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
        let output_path = join_path_components(root, entry_path.components())?;
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("failed to create {}: {error}", output_path.display()))?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        entry.unpack(&output_path).map_err(|error| {
            format!("failed to extract {}: {error}", output_path.display())
        })?;
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
            fs::set_permissions(&output_path, fs::Permissions::from_mode(mode)).map_err(|error| {
                format!("failed to set permissions on {}: {error}", output_path.display())
            })?;
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

fn ensure_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("failed to inspect {}: {error}", path.display())),
        };
        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        if mode & 0o111 == 0 {
            permissions.set_mode(mode | 0o755);
            fs::set_permissions(path, permissions)
                .map_err(|error| format!("failed to set permissions on {}: {error}", path.display()))?;
        }
    }

    Ok(())
}

fn rewrite_program(suggestion: &SuggestedLanguage, program: &Path) -> SuggestedLanguage {
    let mut resolved = suggestion.clone();
    resolved.command[0] = program.display().to_string();
    resolved
}

fn is_command_runnable(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return is_command_runnable_path(Path::new(program));
    }

    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path).any(|entry| is_command_runnable_path(&entry.join(program)))
}

fn is_command_runnable_path(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => is_executable(&metadata.permissions()),
        Ok(_) | Err(_) => false,
    }
}

#[cfg(unix)]
fn is_executable(permissions: &fs::Permissions) -> bool {
    permissions.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_permissions: &fs::Permissions) -> bool {
    true
}

fn join_relative_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    join_path_components(root, Path::new(relative).components())
}

fn join_path_components<'a>(
    root: &Path,
    components: impl IntoIterator<Item = Component<'a>>,
) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for component in components {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(format!(
                    "downloaded package contains unsupported path outside {}",
                    root.display()
                ));
            }
        }
    }
    Ok(path)
}

fn parse_archive_file_spec(file: &str) -> (&str, Option<&str>) {
    match file.split_once(':') {
        Some((archive, directory)) => (archive, Some(directory.trim_end_matches('/'))),
        None => (file, None),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceId {
    Npm { package_name: String, version: String },
    Pypi {
        package_name: String,
        version: String,
        extras: Vec<String>,
    },
    Cargo { package_name: String, version: String },
    Golang { module_path: String, version: String },
    Github { repository: String, version: String },
    Generic { package_name: String, version: String },
    Unsupported { kind: String },
}

fn parse_source_id(source_id: &str) -> Result<SourceId, String> {
    let without_prefix = source_id
        .strip_prefix("pkg:")
        .ok_or_else(|| format!("unsupported Mason package source {source_id}"))?;
    let (package_ref, version_with_qualifiers) = without_prefix
        .rsplit_once('@')
        .ok_or_else(|| format!("unsupported Mason package source {source_id}"))?;
    let (kind, name) = package_ref
        .split_once('/')
        .ok_or_else(|| format!("unsupported Mason package source {source_id}"))?;

    let (version, qualifiers) = split_version_qualifiers(version_with_qualifiers);

    Ok(match kind {
        "npm" => SourceId::Npm {
            package_name: name.to_string(),
            version: version.to_string(),
        },
        "pypi" => SourceId::Pypi {
            package_name: name.to_string(),
            version: version.to_string(),
            extras: parse_pypi_extras(qualifiers),
        },
        "cargo" => SourceId::Cargo {
            package_name: name.to_string(),
            version: version.to_string(),
        },
        "golang" => SourceId::Golang {
            module_path: name.to_string(),
            version: version.to_string(),
        },
        "github" => SourceId::Github {
            repository: name.to_string(),
            version: version.to_string(),
        },
        "generic" => SourceId::Generic {
            package_name: name.to_string(),
            version: version.to_string(),
        },
        _ => SourceId::Unsupported {
            kind: kind.to_string(),
        },
    })
}

fn split_version_qualifiers(version_with_qualifiers: &str) -> (&str, Option<&str>) {
    match version_with_qualifiers.split_once('?') {
        Some((version, qualifiers)) => (version, Some(qualifiers)),
        None => (version_with_qualifiers, None),
    }
}

fn parse_pypi_extras(qualifiers: Option<&str>) -> Vec<String> {
    qualifiers
        .into_iter()
        .flat_map(|qualifiers| url::form_urlencoded::parse(qualifiers.as_bytes()))
        .filter_map(|(key, value)| (key == "extra").then(|| value.into_owned()))
        .collect()
}

fn suggestion_server_name(package: &MasonPackage, program: &str) -> String {
    if package.bin.contains_key(program) {
        program.to_string()
    } else {
        package.name.clone()
    }
}

#[derive(Debug, Serialize)]
struct InstallReceipt {
    package: String,
    source_id: String,
    executable: String,
}

fn write_receipt(
    state: &RuntimeState,
    package: &MasonPackage,
    executable_path: &Path,
) -> Result<PathBuf, String> {
    let receipt = InstallReceipt {
        package: package.name.clone(),
        source_id: package.source.id.clone(),
        executable: executable_path.display().to_string(),
    };
    let path = state.receipt_path(&package.name);
    let Some(parent) = path.parent() else {
        return Err(format!("failed to determine parent directory for {}", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    fs::write(&path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(executable_path.to_path_buf())
}

impl TemplateContext<'_> {
    fn empty() -> Self {
        Self {
            version: "",
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: None,
            source_download_config: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SourceId, WrapperRuntime, is_command_runnable, join_relative_path, materialize_share,
        parse_archive_file_spec, parse_source_id, resolve_program, resolve_program_path,
        rewrite_program, write_wrapper_script,
    };
    use crate::mason_registry::{
        MasonAsset, MasonDownload, MasonNeovim, MasonPackage, MasonSource, OneOrMany,
    };
    use crate::mason_template::TemplateContext;
    use crate::runtime_state::RuntimeState;
    use crate::suggest::SuggestedLanguage;
    use std::collections::BTreeMap;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
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
                "lsp-cli-mason-resolve-test-{}-{}",
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

    fn pyright_package() -> MasonPackage {
        MasonPackage {
            name: "pyright".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:npm/pyright@1.1.409".to_string(),
                extra_packages: Vec::new(),
                asset: None,
                download: None,
            },
            bin: BTreeMap::from([(
                "pyright-langserver".to_string(),
                "npm:pyright-langserver".to_string(),
            )]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("pyright".to_string()),
            },
        }
    }

    fn rust_analyzer_package() -> MasonPackage {
        MasonPackage {
            name: "rust-analyzer".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:github/rust-lang/rust-analyzer@2026-04-27".to_string(),
                extra_packages: Vec::new(),
                asset: Some(OneOrMany::Many(vec![MasonAsset {
                    target: Some(OneOrMany::One("linux_x64_gnu".to_string())),
                    file: OneOrMany::One("rust-analyzer-x86_64-unknown-linux-gnu.gz".to_string()),
                    bin: Some("rust-analyzer-x86_64-unknown-linux-gnu".to_string()),
                }])),
                download: None,
            },
            bin: BTreeMap::from([(
                "rust-analyzer".to_string(),
                "{{source.asset.bin}}".to_string(),
            )]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("rust_analyzer".to_string()),
            },
        }
    }

    fn generic_package() -> MasonPackage {
        MasonPackage {
            name: "bzl".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:generic/bzl@v1.4.22".to_string(),
                extra_packages: Vec::new(),
                asset: None,
                download: Some(OneOrMany::Many(vec![MasonDownload {
                    target: Some(OneOrMany::One("linux_x64".to_string())),
                    files: BTreeMap::from([(
                        "bzl".to_string(),
                        "https://example.invalid/linux_amd64/{{version}}/bzl".to_string(),
                    )]),
                    bin: Some("bzl".to_string()),
                    config: None,
                }])),
            },
            bin: BTreeMap::from([(
                "bzl".to_string(),
                "{{source.download.bin}}".to_string(),
            )]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("bzl".to_string()),
            },
        }
    }

    fn jdtls_package() -> MasonPackage {
        MasonPackage {
            name: "jdtls".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:generic/eclipse/eclipse.jdt.ls@v1.0.0".to_string(),
                extra_packages: Vec::new(),
                asset: None,
                download: Some(OneOrMany::Many(vec![MasonDownload {
                    target: Some(OneOrMany::One("linux".to_string())),
                    files: BTreeMap::from([(
                        "jdtls.tar.gz".to_string(),
                        "https://example.invalid/jdtls.tar.gz".to_string(),
                    )]),
                    bin: None,
                    config: Some("config_linux/".to_string()),
                }])),
            },
            bin: BTreeMap::from([("jdtls".to_string(), "python:bin/jdtls".to_string())]),
            share: BTreeMap::from([
                ("jdtls/plugins/".to_string(), "plugins/".to_string()),
                (
                    "jdtls/config/".to_string(),
                    "{{source.download.config}}".to_string(),
                ),
            ]),
            neovim: MasonNeovim {
                lspconfig: Some("jdtls".to_string()),
            },
        }
    }

    fn suggestion() -> SuggestedLanguage {
        SuggestedLanguage {
            config_id: "pyright".to_string(),
            languages: vec!["python".to_string()],
            server: "pyright-langserver".to_string(),
            command: vec!["pyright-langserver".to_string(), "--stdio".to_string()],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
        }
    }

    #[test]
    fn parses_supported_source_ids() {
        assert_eq!(
            parse_source_id("pkg:npm/pyright@1.1.409").expect("npm source should parse"),
            SourceId::Npm {
                package_name: "pyright".to_string(),
                version: "1.1.409".to_string(),
            }
        );
        assert_eq!(
            parse_source_id("pkg:pypi/jedi-language-server@0.46.0")
                .expect("pypi source should parse"),
            SourceId::Pypi {
                package_name: "jedi-language-server".to_string(),
                version: "0.46.0".to_string(),
                extras: Vec::new(),
            }
        );
        assert_eq!(
            parse_source_id("pkg:pypi/python-lsp-server@1.14.0?extra=all")
                .expect("pypi source with extras should parse"),
            SourceId::Pypi {
                package_name: "python-lsp-server".to_string(),
                version: "1.14.0".to_string(),
                extras: vec!["all".to_string()],
            }
        );
        assert_eq!(
            parse_source_id("pkg:cargo/asm-lsp@0.10.1").expect("cargo source should parse"),
            SourceId::Cargo {
                package_name: "asm-lsp".to_string(),
                version: "0.10.1".to_string(),
            }
        );
        assert_eq!(
            parse_source_id("pkg:golang/golang.org/x/tools/gopls@v0.21.1")
                .expect("golang source should parse"),
            SourceId::Golang {
                module_path: "golang.org/x/tools/gopls".to_string(),
                version: "v0.21.1".to_string(),
            }
        );
    }

    #[test]
    fn preserves_unsupported_source_kind() {
        assert_eq!(
            parse_source_id("pkg:gem/solargraph@0.50.0").expect("gem source should parse"),
            SourceId::Unsupported {
                kind: "gem".to_string(),
            }
        );
    }

    #[test]
    fn resolves_npm_program_path() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));

        assert_eq!(
            resolve_program_path(
                &pyright_package(),
                "pyright-langserver",
                &state,
                &TemplateContext::empty(),
            )
            .expect("managed path should exist"),
            state
                .package_dir("pyright")
                .join("node_modules")
                .join(".bin")
                .join("pyright-langserver")
        );
    }

    #[test]
    fn resolves_github_template_program_path() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));
        let package = rust_analyzer_package();
        let context = TemplateContext {
            version: "2026-04-27",
            source_asset_bin: Some("rust-analyzer-x86_64-unknown-linux-gnu"),
            source_asset_file: Some("rust-analyzer-x86_64-unknown-linux-gnu.gz"),
            source_download_bin: None,
            source_download_config: None,
        };

        assert_eq!(
            resolve_program_path(&package, "rust-analyzer", &state, &context)
                .expect("github path should resolve"),
            state
                .package_dir("rust-analyzer")
                .join("rust-analyzer-x86_64-unknown-linux-gnu")
        );
    }

    #[test]
    fn resolves_generic_template_program_path() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));
        let context = TemplateContext {
            version: "v1.4.22",
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: Some("bzl"),
            source_download_config: None,
        };

        assert_eq!(
            resolve_program_path(&generic_package(), "bzl", &state, &context)
                .expect("generic path should resolve"),
            state.package_dir("bzl").join("bzl")
        );
    }

    #[test]
    fn resolves_python_wrapper_program_path() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));
        let package = jdtls_package();

        assert_eq!(
            resolve_program_path(&package, "jdtls", &state, &TemplateContext::empty())
                .expect("wrapper path should resolve"),
            state.bin_dir().join("jdtls")
        );
        let resolved = resolve_program(&package, "jdtls", &state, &TemplateContext::empty())
            .expect("wrapper program should resolve");

        match resolved {
            super::ResolvedProgram::Wrapper(wrapper) => {
                assert_eq!(wrapper.launcher_path, state.bin_dir().join("jdtls"));
                assert_eq!(wrapper.target_path, state.package_dir("jdtls").join("bin").join("jdtls"));
                assert_eq!(wrapper.runtime, WrapperRuntime::Python);
            }
            super::ResolvedProgram::Direct(_) => panic!("expected wrapper program"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn writes_wrapper_script_for_python_runtime() {
        let dir = TestDir::new();
        let launcher = dir.path().join("jdtls");
        let target = dir.path().join("package/bin/jdtls");
        fs::create_dir_all(target.parent().expect("parent should exist"))
            .expect("parent dirs should be created");
        fs::write(&target, b"print('ok')\n").expect("target should be written");

        write_wrapper_script(&launcher, WrapperRuntime::Python, &target)
            .expect("wrapper should be written");
        super::ensure_executable(&launcher).expect("wrapper should be executable");

        let contents = fs::read_to_string(&launcher).expect("wrapper should be readable");
        assert!(contents.contains("exec python3"));
        assert!(contents.contains(&target.display().to_string()));
        assert!(super::is_command_runnable_path(&launcher));
    }

    #[test]
    fn materializes_share_mappings() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));
        state.ensure_dirs().expect("state dirs should be created");
        let package = jdtls_package();
        let package_dir = state.package_dir("jdtls");
        fs::create_dir_all(package_dir.join("plugins")).expect("plugins dir should be created");
        fs::create_dir_all(package_dir.join("config_linux"))
            .expect("config dir should be created");
        fs::write(package_dir.join("plugins").join("launcher.jar"), b"jar")
            .expect("plugin should be written");
        fs::write(package_dir.join("config_linux").join("config.ini"), b"cfg")
            .expect("config should be written");

        let context = TemplateContext {
            version: "v1.0.0",
            source_asset_bin: None,
            source_asset_file: None,
            source_download_bin: None,
            source_download_config: Some("config_linux/"),
        };
        materialize_share(&state, &package, &context).expect("share should materialize");

        assert!(state
            .share_dir()
            .join("jdtls/plugins/launcher.jar")
            .is_file());
        assert!(state
            .share_dir()
            .join("jdtls/config/config.ini")
            .is_file());
    }

    #[test]
    fn parses_archive_file_spec() {
        assert_eq!(
            parse_archive_file_spec("lua-language-server-3.18.2-linux-x64.tar.gz:libexec/"),
            ("lua-language-server-3.18.2-linux-x64.tar.gz", Some("libexec"))
        );
        assert_eq!(parse_archive_file_spec("clangd-linux-22.1.0.zip"), ("clangd-linux-22.1.0.zip", None));
    }

    #[test]
    fn rejects_parent_directory_paths() {
        let dir = TestDir::new();
        let error = join_relative_path(dir.path(), "../escape").expect_err("parent path should fail");

        assert!(error.contains("outside"));
    }

    #[test]
    fn rewrites_detect_command_program() {
        let rewritten = rewrite_program(&suggestion(), &PathBuf::from("/tmp/pyright-langserver"));

        assert_eq!(
            rewritten.command,
            vec![
                "/tmp/pyright-langserver".to_string(),
                "--stdio".to_string(),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn detects_runnable_command_on_path() {
        let dir = TestDir::new();
        let executable = dir.path().join("pyright-langserver");
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("file should be written");
        let mut permissions = fs::metadata(&executable)
            .expect("metadata should be available")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).expect("permissions should be updated");

        let original_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", dir.path()) };
        let detected = is_command_runnable("pyright-langserver");
        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert!(detected);
    }
}
