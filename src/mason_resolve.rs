use crate::mason_registry::{MasonPackage, MasonRegistry};
use crate::runtime_state::{RuntimeState, default_runtime_state_root};
use crate::suggest::SuggestedLanguage;
use serde::Serialize;
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    if let Some(path) = managed_program_path(state, package, program)
        && is_command_runnable_path(&path)
    {
        return Ok(path);
    }

    match parse_source_id(&package.source.id)? {
        SourceId::Npm { package_name, version } => {
            install_npm_package(state, package, &package_name, &version, program)
        }
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
    if !is_command_runnable("npm") {
        return Err(format!(
            "cannot install {} because npm is not available in $PATH",
            suggestion_server_name(package, program)
        ));
    }

    state.ensure_dirs()?;
    let install_dir = state.package_dir(&package.name);
    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("failed to create {}: {error}", install_dir.display()))?;

    let install_spec = format!("{package_name}@{version}");
    let status = Command::new("npm")
        .arg("install")
        .arg("--no-package-lock")
        .arg("--prefix")
        .arg(&install_dir)
        .arg(&install_spec)
        .args(&package.source.extra_packages)
        .output()
        .map_err(|error| format!("cannot install {} because npm could not start: {error}", package.name))?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            "npm install failed".to_string()
        } else {
            stderr.lines().next().unwrap_or("npm install failed").to_string()
        };
        return Err(format!(
            "cannot install {} because npm failed: {detail}",
            package.name
        ));
    }

    let executable_path = managed_program_path(state, package, program).ok_or_else(|| {
        format!(
            "cannot install {} because Mason does not expose executable {program}",
            package.name
        )
    })?;

    if !is_command_runnable_path(&executable_path) {
        return Err(format!(
            "cannot install {} because npm did not produce a runnable {program} executable",
            package.name
        ));
    }

    write_receipt(
        &state.receipt_path(&package.name),
        &InstallReceipt {
            package: package.name.clone(),
            source_id: package.source.id.clone(),
            executable: executable_path.display().to_string(),
        },
    )?;

    Ok(executable_path)
}

fn managed_program_path(state: &RuntimeState, package: &MasonPackage, program: &str) -> Option<PathBuf> {
    let target = package.bin.get(program)?;
    let relative = target.strip_prefix("npm:")?;
    Some(
        state
            .package_dir(&package.name)
            .join("node_modules")
            .join(".bin")
            .join(relative),
    )
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceId {
    Npm { package_name: String, version: String },
    Unsupported { kind: String },
}

fn parse_source_id(source_id: &str) -> Result<SourceId, String> {
    let without_prefix = source_id
        .strip_prefix("pkg:")
        .ok_or_else(|| format!("unsupported Mason package source {source_id}"))?;
    let (package_ref, version) = without_prefix
        .rsplit_once('@')
        .ok_or_else(|| format!("unsupported Mason package source {source_id}"))?;
    let (kind, name) = package_ref
        .split_once('/')
        .ok_or_else(|| format!("unsupported Mason package source {source_id}"))?;

    if kind == "npm" {
        return Ok(SourceId::Npm {
            package_name: name.to_string(),
            version: version.to_string(),
        });
    }

    Ok(SourceId::Unsupported {
        kind: kind.to_string(),
    })
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

fn write_receipt(path: &Path, receipt: &InstallReceipt) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!("failed to determine parent directory for {}", path.display()));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(receipt)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{
        SourceId, is_command_runnable, managed_program_path, parse_source_id, rewrite_program,
    };
    use crate::mason_registry::{MasonNeovim, MasonPackage, MasonSource};
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
            },
            bin: BTreeMap::from([(
                "pyright-langserver".to_string(),
                "npm:pyright-langserver".to_string(),
            )]),
            neovim: MasonNeovim {
                lspconfig: Some("pyright".to_string()),
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
    fn parses_npm_source_id() {
        assert_eq!(
            parse_source_id("pkg:npm/pyright@1.1.409").expect("npm source should parse"),
            SourceId::Npm {
                package_name: "pyright".to_string(),
                version: "1.1.409".to_string(),
            }
        );
    }

    #[test]
    fn preserves_unsupported_source_kind() {
        assert_eq!(
            parse_source_id("pkg:github/rust-lang/rust-analyzer@2026-04-27")
                .expect("github source should parse"),
            SourceId::Unsupported {
                kind: "github".to_string(),
            }
        );
    }

    #[test]
    fn computes_managed_npm_program_path() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));

        assert_eq!(
            managed_program_path(&state, &pyright_package(), "pyright-langserver")
                .expect("managed path should exist"),
            state
                .package_dir("pyright")
                .join("node_modules")
                .join(".bin")
                .join("pyright-langserver")
        );
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
        // Safety: tests in this crate are not asserting on PATH concurrently.
        unsafe { std::env::set_var("PATH", dir.path()) };
        let detected = is_command_runnable("pyright-langserver");
        match original_path {
            Some(path) => {
                // Safety: restoring prior test process state.
                unsafe { std::env::set_var("PATH", path) };
            }
            None => {
                // Safety: restoring prior test process state.
                unsafe { std::env::remove_var("PATH") };
            }
        }

        assert!(detected);
    }
}
