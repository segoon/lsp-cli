use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub fn receipt_path(&self, package: &str) -> PathBuf {
        self.receipts_dir().join(format!("{package}.json"))
    }

    pub fn ensure_dirs(&self) -> Result<(), String> {
        for path in [
            self.root().to_path_buf(),
            self.registry_dir(),
            self.packages_dir(),
            self.bin_dir(),
            self.share_dir(),
            self.receipts_dir(),
        ] {
            fs::create_dir_all(&path)
                .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
        }

        Ok(())
    }
}

pub fn default_runtime_state_root() -> Result<PathBuf, String> {
    let home = env::var_os("HOME").map(PathBuf::from);
    choose_runtime_state_root(home.as_deref())
}

fn choose_runtime_state_root(home: Option<&Path>) -> Result<PathBuf, String> {
    home.map(|path| path.join(".local/share/lsp-cli"))
        .ok_or_else(|| "could not resolve runtime state root because $HOME is not set".to_string())
}

#[cfg(test)]
mod tests {
    use super::{RuntimeState, choose_runtime_state_root};
    use std::fs;
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
                "lsp-cli-runtime-state-test-{}-{}",
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

    #[test]
    fn resolves_runtime_state_under_home() {
        let home = TestDir::new();

        assert_eq!(
            choose_runtime_state_root(Some(home.path())).expect("root should resolve"),
            home.path().join(".local/share/lsp-cli")
        );
    }

    #[test]
    fn errors_without_home() {
        let error = choose_runtime_state_root(None).expect_err("missing home should fail");

        assert!(error.contains("runtime state root"));
    }

    #[test]
    fn creates_expected_runtime_directories() {
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));

        state.ensure_dirs().expect("directories should be created");

        assert!(state.registry_dir().is_dir());
        assert!(state.packages_dir().is_dir());
        assert!(state.bin_dir().is_dir());
        assert!(state.share_dir().is_dir());
        assert!(state.receipts_dir().is_dir());
    }
}
