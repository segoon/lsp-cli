use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::dependencies::ManagedDependencies;

use super::E2eContext;

pub(crate) const REGISTRY_SNAPSHOT_ENV: &str = "E2E_MASON_REGISTRY_SNAPSHOT";
pub(super) const INSTALL_PATH_ENV: &str = "LSP_CLI_INSTALL_PATH";
pub(super) const CARGO_HOME_ENV: &str = "LSP_CLI_INSTALL_CARGO_HOME";
pub(super) const RUSTUP_HOME_ENV: &str = "LSP_CLI_INSTALL_RUSTUP_HOME";
pub(super) const RUSTUP_TOOLCHAIN_ENV: &str = "LSP_CLI_INSTALL_RUSTUP_TOOLCHAIN";
const RUNTIME_STATE_DIR: &str = ".local/share/lsp-cli";
const REGISTRY_FILES: [&str; 2] = ["registry.json", "metadata.json"];

#[derive(Deserialize)]
struct RegistryMetadata {
    release_tag: String,
    refreshed_at_epoch_seconds: u64,
    digest: Option<String>,
}

impl E2eContext {
    pub(crate) fn new_for_real_server(dependencies: &ManagedDependencies) -> Result<Self, String> {
        let mut context = Self::new()
            .map_err(|error| format!("failed to create an isolated E2E context: {error}"))?;
        let host_home = std::env::var_os("HOME").map(PathBuf::from);
        context.install_path = Some(dependencies.install_path().to_os_string());
        context.cargo_home = tool_home(
            std::env::var_os(CARGO_HOME_ENV),
            host_home.as_deref(),
            ".cargo",
        );
        context.rustup_home = tool_home(
            std::env::var_os(RUSTUP_HOME_ENV),
            host_home.as_deref(),
            ".rustup",
        );
        context.rustup_toolchain = std::env::var_os(RUSTUP_TOOLCHAIN_ENV);
        if let Some(snapshot) = std::env::var_os(REGISTRY_SNAPSHOT_ENV) {
            context.seed_registry_snapshot(Path::new(&snapshot))?;
        }
        Ok(context)
    }

    pub(super) fn process_path(&self) -> OsString {
        self.bin_dir.as_os_str().to_owned()
    }

    fn seed_registry_snapshot(&self, source: &Path) -> Result<(), String> {
        validate_registry_snapshot(source)?;
        let destination = self.registry_dir();
        fs::create_dir_all(&destination).map_err(|error| {
            format!(
                "failed to create Mason registry snapshot destination {}: {error}",
                destination.display()
            )
        })?;
        for name in REGISTRY_FILES {
            let source_file = source.join(name);
            let destination_file = destination.join(name);
            fs::copy(&source_file, &destination_file).map_err(|error| {
                format!(
                    "failed to copy Mason registry snapshot {} to {}: {error}",
                    source_file.display(),
                    destination_file.display()
                )
            })?;
        }
        Ok(())
    }

    fn registry_dir(&self) -> std::path::PathBuf {
        self.home.join(RUNTIME_STATE_DIR).join("registry")
    }

    #[cfg(test)]
    pub(super) fn use_install_path(&mut self, path: OsString) {
        self.install_path = Some(path);
    }

    #[cfg(test)]
    pub(super) fn use_rust_toolchain_environment(
        &mut self,
        cargo_home: OsString,
        rustup_home: OsString,
        rustup_toolchain: OsString,
    ) {
        self.cargo_home = Some(cargo_home);
        self.rustup_home = Some(rustup_home);
        self.rustup_toolchain = Some(rustup_toolchain);
    }

    #[cfg(test)]
    pub(super) fn seed_test_registry_snapshot(&self, source: &Path) -> Result<(), String> {
        self.seed_registry_snapshot(source)
    }

    #[cfg(test)]
    pub(super) fn test_registry_dir(&self) -> std::path::PathBuf {
        self.registry_dir()
    }
}

fn tool_home(
    explicit: Option<OsString>,
    host_home: Option<&Path>,
    default_directory: &str,
) -> Option<OsString> {
    explicit.or_else(|| {
        let path = host_home?.join(default_directory);
        path.is_dir().then(|| path.into_os_string())
    })
}

fn validate_registry_snapshot(source: &Path) -> Result<(), String> {
    let registry_path = source.join(REGISTRY_FILES[0]);
    let registry = read_json::<Vec<Value>>(&registry_path)?;
    if registry.is_empty() {
        return Err(format!(
            "Mason registry snapshot {} contains no packages",
            registry_path.display()
        ));
    }

    let metadata_path = source.join(REGISTRY_FILES[1]);
    let metadata = read_json::<RegistryMetadata>(&metadata_path)?;
    if metadata.release_tag.is_empty() || metadata.refreshed_at_epoch_seconds == 0 {
        return Err(format!(
            "Mason registry snapshot metadata {} is incomplete",
            metadata_path.display()
        ));
    }
    if metadata
        .digest
        .as_deref()
        .is_some_and(|digest| !digest.starts_with("sha256:"))
    {
        return Err(format!(
            "Mason registry snapshot metadata {} has an unsupported digest",
            metadata_path.display()
        ));
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read Mason registry snapshot {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "failed to parse Mason registry snapshot {}: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn context() -> E2eContext {
        E2eContext::new().expect("E2E context should initialize")
    }

    fn write_valid_snapshot(directory: &Path) -> Vec<u8> {
        fs::create_dir(directory).expect("snapshot directory should be created");
        let registry = br#"[{"name":"server"}]"#.to_vec();
        fs::write(directory.join(REGISTRY_FILES[0]), &registry)
            .expect("registry should be written");
        let refreshed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_secs();
        let metadata = serde_json::json!({
            "release_tag": "2026-09-16",
            "refreshed_at_epoch_seconds": refreshed,
            "digest": "sha256:0123"
        });
        fs::write(
            directory.join(REGISTRY_FILES[1]),
            serde_json::to_vec(&metadata).expect("metadata should serialize"),
        )
        .expect("metadata should be written");
        registry
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("executable fixture should be written");
        let mut permissions = fs::metadata(path)
            .expect("executable fixture should have metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .expect("executable fixture permissions should be updated");
    }

    #[test]
    fn tool_home_prefers_explicit_value_and_requires_an_existing_default() {
        let context = context();
        let explicit = OsString::from("explicit-home");
        let host_home = context.workspace.join("host-home");
        let default = host_home.join(".cargo");
        fs::create_dir_all(&default).expect("default tool home should be created");

        assert_eq!(
            tool_home(Some(explicit.clone()), Some(&host_home), ".cargo"),
            Some(explicit)
        );
        assert_eq!(
            tool_home(None, Some(&host_home), ".cargo"),
            Some(default.into_os_string())
        );
        assert_eq!(tool_home(None, Some(&context.workspace), ".rustup"), None);
        assert_eq!(tool_home(None, None, ".cargo"), None);
    }

    #[test]
    fn real_server_isolates_server_lookup_and_preserves_installer_state() {
        let mut context = context();
        let host_paths = [
            context.workspace.join("host-one"),
            context.workspace.join("host-two"),
        ];
        let install_path = std::env::join_paths(&host_paths).expect("host PATH should join");
        context.use_install_path(install_path.clone());
        let cargo_home = context.workspace.join("cargo-home").into_os_string();
        let rustup_home = context.workspace.join("rustup-home").into_os_string();
        let toolchain = OsString::from("stable");
        context.use_rust_toolchain_environment(
            cargo_home.clone(),
            rustup_home.clone(),
            toolchain.clone(),
        );

        let actual = std::env::split_paths(&context.process_path()).collect::<Vec<_>>();
        let command = context.command();
        let environment = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsString::from)))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(actual, [context.bin_dir.clone()]);
        assert_eq!(
            environment[std::ffi::OsStr::new(INSTALL_PATH_ENV)],
            Some(install_path)
        );
        assert_eq!(
            environment[std::ffi::OsStr::new(CARGO_HOME_ENV)],
            Some(cargo_home)
        );
        assert_eq!(
            environment[std::ffi::OsStr::new(RUSTUP_HOME_ENV)],
            Some(rustup_home)
        );
        assert_eq!(
            environment[std::ffi::OsStr::new(RUSTUP_TOOLCHAIN_ENV)],
            Some(toolchain)
        );
    }

    #[test]
    fn registry_snapshot_is_copied_into_isolated_home() {
        let context = context();
        let source = context.workspace.join("snapshot");
        let registry = write_valid_snapshot(&source);

        context
            .seed_test_registry_snapshot(&source)
            .expect("snapshot should be seeded");

        assert_eq!(
            fs::read(context.test_registry_dir().join(REGISTRY_FILES[0]))
                .expect("seeded registry should be readable"),
            registry
        );
        assert!(
            context
                .test_registry_dir()
                .join(REGISTRY_FILES[1])
                .is_file()
        );
    }

    #[test]
    fn incomplete_registry_snapshot_fails_before_copying() {
        let context = context();
        let source = context.workspace.join("snapshot");
        fs::create_dir(&source).expect("snapshot directory should be created");
        fs::write(source.join(REGISTRY_FILES[0]), br#"[{"name":"server"}]"#)
            .expect("registry should be written");

        let error = context
            .seed_test_registry_snapshot(&source)
            .expect_err("incomplete snapshot should fail");

        assert!(error.contains("failed to read Mason registry snapshot"));
        assert!(!context.test_registry_dir().exists());
    }

    #[test]
    fn malformed_registry_snapshot_fails_before_copying() {
        let context = context();
        let source = context.workspace.join("snapshot");
        write_valid_snapshot(&source);
        fs::write(source.join(REGISTRY_FILES[0]), b"{not-json]")
            .expect("invalid registry should be written");

        let error = context
            .seed_test_registry_snapshot(&source)
            .expect_err("malformed snapshot should fail");

        assert!(error.contains("failed to parse Mason registry snapshot"));
        assert!(!context.test_registry_dir().exists());
    }

    #[cfg(unix)]
    #[test]
    fn staged_runtime_runs_env_shebang_without_exposing_ambient_programs() {
        let mut context = context();
        let host_tools = context.workspace.join("host-tools");
        fs::create_dir(&host_tools).expect("host tool directory should be created");
        let node = host_tools.join("node");
        let ambient = host_tools.join("ambient-lsp");
        let server = context.workspace.join("server");
        write_executable(&node, "#!/bin/sh\nprintf 'runtime-ok\\n'\n");
        write_executable(&ambient, "#!/bin/sh\nexit 0\n");
        write_executable(&server, "#!/usr/bin/env node\n");
        context.use_install_path(host_tools.as_os_str().to_owned());
        let resolver = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!("printf '%s\\n' {}", node.display()),
        ];

        context
            .stage_host_program("node", &resolver, Duration::from_secs(1))
            .expect("runtime should be staged");
        context
            .stage_host_program("node", &resolver, Duration::from_secs(1))
            .expect("staging the same runtime should be idempotent");
        context
            .stage_program("server", &server)
            .expect("server should be staged");

        let output = context
            .run_test_program(context.bin_dir.join("server"), &[], Duration::from_secs(1))
            .expect("staged server should run");
        output
            .ensure_success()
            .expect("staged runtime should execute");
        assert_eq!(output.stdout_text(), "runtime-ok\n");

        let ambient_lookup = context
            .run_test_program(
                "/bin/sh",
                &["-c", "command -v ambient-lsp"],
                Duration::from_secs(1),
            )
            .expect("ambient lookup should finish");
        assert!(ambient_lookup.ensure_success().is_err());
    }
}
