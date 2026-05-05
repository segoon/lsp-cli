use crate::env_vars;
use crate::fs as path_fs;
use crate::mason::registry::MasonPackage;
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use crate::suggest::SuggestedLanguage;
use serde::Serialize;
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

const MASON_PATH_SEPARATOR: char = '/';

// A resolved launcher path is either the executable itself or a generated wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedProgram {
    Direct(PathBuf),
    Wrapper(WrapperProgram),
}

impl ResolvedProgram {
    pub(crate) fn executable_path(&self) -> &Path {
        match self {
            Self::Direct(path) => path,
            Self::Wrapper(wrapper) => &wrapper.launcher_path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WrapperProgram {
    launcher_path: PathBuf,
    target_path: PathBuf,
    runtime: WrapperRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WrapperRuntime {
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

pub(crate) fn resolve_program(
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

pub(crate) fn is_resolved_program_runnable(program: &ResolvedProgram) -> bool {
    match program {
        ResolvedProgram::Direct(path) => is_command_runnable_path(path),
        ResolvedProgram::Wrapper(wrapper) => {
            is_command_runnable_path(&wrapper.launcher_path)
                && wrapper.target_path.is_file()
                && is_command_runnable(wrapper.runtime.command_name())
        }
    }
}

pub(crate) fn finalize_install(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
    resolved_program: &ResolvedProgram,
    context: &TemplateContext<'_>,
    failure_reason: &str,
) -> Result<PathBuf, String> {
    materialize_share(state, package, context)?;
    ensure_resolved_program(package, program, resolved_program)?;

    if !is_resolved_program_runnable(resolved_program) {
        return Err(format!(
            "cannot install {} because {failure_reason} {program} executable",
            package.name
        ));
    }

    write_receipt(state, package, resolved_program.executable_path())
}

pub(crate) fn rewrite_program(suggestion: &SuggestedLanguage, program: &Path) -> SuggestedLanguage {
    let mut resolved = suggestion.clone();
    resolved.command[0] = program.display().to_string();
    resolved
}

pub(crate) fn is_command_runnable(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return is_command_runnable_path(Path::new(program));
    }

    let Some(path) = env_vars::path() else {
        return false;
    };

    env::split_paths(&path).any(|entry| is_command_runnable_path(&entry.join(program)))
}

pub(crate) fn join_relative_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    join_path_components(root, Path::new(relative).components())
}

fn ensure_resolved_program(
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
            write_wrapper_script(
                &wrapper.launcher_path,
                wrapper.runtime,
                &wrapper.target_path,
            )?;
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
        let target_is_dir = rendered_target.ends_with(MASON_PATH_SEPARATOR);
        let source_is_dir = rendered_source.ends_with(MASON_PATH_SEPARATOR);
        let share_path =
            join_relative_path(
                &state.share_dir(),
                rendered_target.trim_end_matches(MASON_PATH_SEPARATOR),
            )?;
        let package_path = join_relative_path(
            &state.package_dir(&package.name),
            rendered_source.trim_end_matches(MASON_PATH_SEPARATOR),
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
    #[cfg(unix)]
    {
        let contents = format!("#!/bin/sh\n{}\n", runtime.script_line(target_path));
        write_file(launcher_path, contents.as_bytes())
    }

    #[cfg(not(unix))]
    {
        let _ = (runtime, target_path);
        Err(format!(
            "failed to create wrapper launcher {} because wrapper scripts are only implemented on Unix",
            launcher_path.display()
        ))
    }
}

fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    let bytes = path_fs::read(source)?;
    write_file(target, &bytes)
}

fn copy_directory_contents(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = path_fs::metadata(source)?;
    if !metadata.is_dir() {
        return Err(format!("expected directory at {}", source.display()));
    }

    path_fs::create_dir_all(target)?;
    for entry in path_fs::read_dir(source)? {
        let entry = entry.map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = entry.metadata().map_err(|error| {
            format!("failed to inspect {}: {error}", source_path.display())
        })?;
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

fn suggestion_server_name(package: &MasonPackage, program: &str) -> String {
    if package.bin.contains_key(program) {
        program.to_string()
    } else {
        package.name.clone()
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!(
            "failed to determine parent directory for {}",
            path.display()
        ));
    };
    path_fs::create_dir_all(parent)?;
    path_fs::write(path, bytes)
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
        let hardened_mode = (mode | 0o111) & !0o022;
        if hardened_mode != mode {
            permissions.set_mode(hardened_mode);
            path_fs::set_permissions(path, permissions)?;
        }
    }

    Ok(())
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
        return Err(format!(
            "failed to determine parent directory for {}",
            path.display()
        ));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    fs::write(&path, bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(executable_path.to_path_buf())
}

#[cfg(test)]
mod tests;
