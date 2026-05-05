use crate::env_vars;
use crate::error::{Error, Result};
use crate::hash::encode_hex;
use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

// Per-user root for lsp-cli runtime state: registry cache, installs, logs, and receipts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeState {
    root: PathBuf,
}

impl RuntimeState {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn registry_dir(&self) -> PathBuf {
        self.root.join("registry")
    }

    #[must_use]
    pub fn registry_json_path(&self) -> PathBuf {
        self.registry_dir().join("registry.json")
    }

    #[must_use]
    pub fn registry_metadata_path(&self) -> PathBuf {
        self.registry_dir().join("metadata.json")
    }

    #[must_use]
    pub fn packages_dir(&self) -> PathBuf {
        self.root.join("packages")
    }

    #[must_use]
    pub fn package_dir(&self, package: &str) -> PathBuf {
        self.packages_dir().join(package)
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    #[must_use]
    pub fn share_dir(&self) -> PathBuf {
        self.root.join("share")
    }

    #[must_use]
    pub fn receipts_dir(&self) -> PathBuf {
        self.root.join("receipts")
    }

    #[must_use]
    pub fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    #[must_use]
    pub fn log_path(&self) -> PathBuf {
        self.root.join("lsp-cli.log")
    }

    #[must_use]
    pub fn receipt_path(&self, package: &str) -> PathBuf {
        self.receipts_dir().join(format!("{package}.json"))
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for path in [
            self.root().to_path_buf(),
            self.registry_dir(),
            self.packages_dir(),
            self.bin_dir(),
            self.share_dir(),
            self.receipts_dir(),
            self.data_dir(),
        ] {
            fs::create_dir_all(&path).map_err(|error| {
                Error::unexpected(format!("failed to create {}: {error}", path.display()))
            })?;
        }

        Ok(())
    }
}

pub fn default_runtime_state_root() -> Result<PathBuf> {
    let home = env_vars::home();
    choose_runtime_state_root(home.as_deref())
}

pub fn default_daemon_root() -> Result<PathBuf> {
    let runtime_dir = env_vars::xdg_runtime();
    choose_daemon_root(runtime_dir.as_deref())
}

#[must_use]
pub fn daemon_socket_path(
    daemon_root: &Path,
    workspace_root: &Path,
    server_name: &str,
    command: &[String],
) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(workspace_root.display().to_string().as_bytes());
    hasher.update([0]);
    for argument in command {
        hasher.update(argument.as_bytes());
        hasher.update([0]);
    }

    let digest = encode_hex(&hasher.finalize());
    let slug = sanitize_daemon_socket_component(server_name);
    daemon_root.join(format!("{}-{}.sock", slug, &digest[..24]))
}

pub fn daemon_socket_paths(daemon_root: &Path) -> Result<Vec<PathBuf>> {
    if !daemon_root.exists() {
        return Ok(Vec::new());
    }

    let mut paths = fs::read_dir(daemon_root)
        .map_err(|error| {
            Error::unexpected(format!("failed to read {}: {error}", daemon_root.display()))
        })?
        .filter_map(|entry| {
            let Ok(entry) = entry else {
                return None;
            };
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                return None;
            };
            if path.extension().and_then(|value| value.to_str()) == Some("sock")
                && file_type.is_socket()
            {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn choose_runtime_state_root(home: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = home {
        Ok(path.join(".local/share/lsp-cli"))
    } else {
        Err(Error::unexpected(
            "could not resolve runtime state root because $HOME is not set",
        ))
    }
}

fn choose_daemon_root(runtime_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = runtime_dir {
        Ok(path.join("lsp-cli"))
    } else {
        Err(Error::unexpected(
            "could not resolve daemon socket root because $XDG_RUNTIME_DIR is not set",
        ))
    }
}

fn sanitize_daemon_socket_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "lsp".to_string()
    } else {
        sanitized.chars().take(32).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeState, choose_daemon_root, choose_runtime_state_root, daemon_socket_path,
        daemon_socket_paths, sanitize_daemon_socket_component,
    };
    use crate::test_support::{LOCAL_SHARE_LSP_CLI, TestDir};
    use std::fs;
    use std::os::unix::net::UnixListener;

    #[test]
    fn resolves_runtime_state_under_home() {
        let home = TestDir::new("runtime-state");

        assert_eq!(
            choose_runtime_state_root(Some(home.path())).expect("root should resolve"),
            home.path().join(LOCAL_SHARE_LSP_CLI)
        );
    }

    #[test]
    fn errors_without_home() {
        let error = choose_runtime_state_root(None).expect_err("missing home should fail");

        assert!(error.contains("runtime state root"));
    }

    #[test]
    fn resolves_daemon_root_under_xdg_runtime_dir() {
        let runtime_dir = TestDir::new("daemon-root");

        assert_eq!(
            choose_daemon_root(Some(runtime_dir.path())).expect("root should resolve"),
            runtime_dir.path().join("lsp-cli")
        );
    }

    #[test]
    fn errors_without_xdg_runtime_dir() {
        let error = choose_daemon_root(None).expect_err("missing runtime dir should fail");

        assert!(error.contains("daemon socket root"));
    }

    #[test]
    fn daemon_socket_path_depends_on_workspace_and_command() {
        let dir = TestDir::new("daemon-root");
        let daemon_root = dir.path().join("runtime");
        let first = daemon_socket_path(
            &daemon_root,
            &dir.path().join("one"),
            "rust-analyzer",
            &["rust-analyzer".to_string()],
        );
        let second = daemon_socket_path(
            &daemon_root,
            &dir.path().join("two"),
            "rust-analyzer",
            &["rust-analyzer".to_string()],
        );
        let third = daemon_socket_path(
            &daemon_root,
            &dir.path().join("one"),
            "rust-analyzer",
            &["rust-analyzer".to_string(), "--stdio".to_string()],
        );

        assert_ne!(first, second);
        assert_ne!(first, third);
    }

    #[test]
    fn stores_global_log_file_under_runtime_root() {
        let dir = TestDir::new("runtime-log-path");
        let state = RuntimeState::new(dir.path().join("state"));

        assert_eq!(state.log_path(), dir.path().join("state/lsp-cli.log"));
    }

    #[test]
    fn lists_only_daemon_socket_paths() {
        let dir = TestDir::new("daemon-socket-list");
        let daemon_root = dir.path().join("runtime");
        {
            fs::create_dir_all(&daemon_root).expect("daemon root should exist");
            let socket_path = daemon_root.join("alpha.sock");
            let _listener = UnixListener::bind(&socket_path).expect("socket should bind");
            fs::write(daemon_root.join("notes.txt"), b"")
                .expect("other placeholder should be written");
            fs::create_dir_all(daemon_root.join("beta.sock"))
                .expect("directory placeholder should be written");

            assert_eq!(
                daemon_socket_paths(&daemon_root).expect("socket listing should succeed"),
                vec![socket_path.clone()]
            );
        }
    }

    #[test]
    fn sanitizes_daemon_socket_component() {
        assert_eq!(
            sanitize_daemon_socket_component("Rust Analyzer"),
            "rust-analyzer"
        );
        assert_eq!(sanitize_daemon_socket_component("***"), "lsp");
    }

    #[test]
    fn creates_expected_runtime_directories() {
        let dir = TestDir::new("runtime-state");
        let state = RuntimeState::new(dir.path().join("state"));

        state.ensure_dirs().expect("directories should be created");

        assert!(state.registry_dir().is_dir());
        assert!(state.packages_dir().is_dir());
        assert!(state.bin_dir().is_dir());
        assert!(state.share_dir().is_dir());
        assert!(state.receipts_dir().is_dir());
    }
}
