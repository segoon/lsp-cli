use std::ffi::OsString;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::E2eContext;

pub(crate) const REGISTRY_SNAPSHOT_ENV: &str = "E2E_MASON_REGISTRY_SNAPSHOT";
const RUNTIME_STATE_DIR: &str = ".local/share/lsp-cli";
const REGISTRY_FILES: [&str; 2] = ["registry.json", "metadata.json"];

#[derive(Deserialize)]
struct RegistryMetadata {
    release_tag: String,
    refreshed_at_epoch_seconds: u64,
    digest: Option<String>,
}

impl E2eContext {
    pub(crate) fn new_for_real_server() -> Result<Self, String> {
        let mut context = Self::new()
            .map_err(|error| format!("failed to create an isolated E2E context: {error}"))?;
        context.host_path = std::env::var_os("PATH");
        if let Some(snapshot) = std::env::var_os(REGISTRY_SNAPSHOT_ENV) {
            context.seed_registry_snapshot(Path::new(&snapshot))?;
        }
        Ok(context)
    }

    pub(super) fn process_path(&self) -> OsString {
        let paths = std::iter::once(self.bin_dir.clone()).chain(
            self.host_path
                .as_deref()
                .into_iter()
                .flat_map(std::env::split_paths),
        );
        std::env::join_paths(paths).expect("existing PATH entries should remain valid")
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
    pub(super) fn use_host_path(&mut self, path: OsString) {
        self.host_path = Some(path);
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

    #[test]
    fn real_server_path_prefers_isolated_bin_and_preserves_host_tools() {
        let mut context = context();
        let host_paths = [
            context.workspace.join("host-one"),
            context.workspace.join("host-two"),
        ];
        context.use_host_path(std::env::join_paths(&host_paths).expect("host PATH should join"));

        let actual = std::env::split_paths(&context.process_path()).collect::<Vec<_>>();

        assert_eq!(
            actual,
            [vec![context.bin_dir.clone()], host_paths.to_vec()].concat()
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
}
