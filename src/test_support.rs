use crate::mason::registry::{MasonDownload, MasonNeovim, MasonPackage, MasonSource, OneOrMany};
use crate::runtime_state::RuntimeState;
use crate::suggest::SuggestedLanguage;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const LOCAL_SHARE_LSP_CLI: &str = ".local/share/lsp-cli";

pub(crate) struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub(crate) fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "lsp-cli-{prefix}-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir should be created");
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn write_file(&self, relative: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dirs should be created");
        }

        fs::write(&path, contents).expect("file should be written");
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct EnvGuard {
    saved: Vec<(String, Option<OsString>)>,
}

impl EnvGuard {
    fn new(vars: &[(&str, OsString)]) -> Self {
        let saved = vars
            .iter()
            .map(|(name, _)| ((*name).to_string(), std::env::var_os(name)))
            .collect::<Vec<_>>();

        for (name, value) in vars {
            // Tests serialize env changes with a global mutex because process env is shared.
            unsafe { std::env::set_var(name, value) };
        }

        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.saved {
            match value {
                Some(value) => {
                    // Tests serialize env changes with a global mutex because process env is shared.
                    unsafe { std::env::set_var(name, value) };
                }
                None => {
                    // Tests serialize env changes with a global mutex because process env is shared.
                    unsafe { std::env::remove_var(name) };
                }
            }
        }
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn env_var(name: &'static str, value: impl AsRef<OsStr>) -> (&'static str, OsString) {
    (name, value.as_ref().to_os_string())
}

pub(crate) fn with_env_vars<T>(vars: &[(&str, OsString)], run: impl FnOnce() -> T) -> T {
    let _lock = env_lock().lock().expect("env lock should be available");
    let _guard = EnvGuard::new(vars);
    run()
}

pub(crate) fn runtime_state_in_home(home: &Path) -> RuntimeState {
    RuntimeState::new(home.join(LOCAL_SHARE_LSP_CLI))
}

pub(crate) fn write_registry(state: &RuntimeState, packages: &[MasonPackage]) {
    fs::create_dir_all(state.registry_dir()).expect("registry dir should be created");
    let bytes = serde_json::to_vec(packages).expect("registry should serialize");
    fs::write(state.registry_json_path(), bytes).expect("registry should be written");
}

pub(crate) fn pyright_package() -> MasonPackage {
    MasonPackage {
        name: "pyright".to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: "pkg:npm/pyright@1.1.409".to_string(),
            extra_packages: Vec::new(),
            asset: None,
            download: None,
            version_overrides: Vec::new(),
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

pub(crate) fn jdtls_package() -> MasonPackage {
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
                man: None,
            }])),
            version_overrides: Vec::new(),
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

pub(crate) fn suggested_language(
    program: &str,
    config_id: &str,
    server: &str,
    language: &str,
) -> SuggestedLanguage {
    SuggestedLanguage {
        config_id: config_id.to_string(),
        languages: vec![language.to_string()],
        server: server.to_string(),
        command: vec![program.to_string(), "--stdio".to_string()],
        workspace_root: PathBuf::from("."),
        wait_for_index: false,
    }
}

#[cfg(unix)]
pub(crate) fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .expect("metadata should be available")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("permissions should be updated");
}
