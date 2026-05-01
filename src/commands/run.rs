use crate::cli::RunArgs;
use crate::commands::common::{analyze_path, select_server};
use crate::config::ConfigStore;
use crate::mason::resolve_detect_suggestions;
use crate::suggest::SuggestedLanguage;
use std::process::Command;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub(super) fn run(args: &RunArgs, config: &ConfigStore) -> Result<String, String> {
    let (detection, suggestions) = analyze_path(&args.path, config)?;
    let server = resolve_server(&detection, &suggestions, args.lsp.as_deref())?;
    let Some(program) = server.command.first() else {
        return Err(format!(
            "selected LSP server {} has an empty command",
            server.server
        ));
    };

    if args.debug {
        eprintln!("LSP server: {}", server.command.join(" "));
    }

    let mut command = Command::new(program);
    command
        .args(&server.command[1..])
        .current_dir(&server.workspace_root);

    #[cfg(unix)]
    {
        Err(format_exec_error(program, &command.exec()))
    }

    #[cfg(not(unix))]
    {
        let _ = command;
        Err("lsp-cli run is only supported on unix-like systems".to_string())
    }
}

fn format_exec_error(program: &str, error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound if !program.contains(std::path::MAIN_SEPARATOR) => {
            format!("LSP server executable `{program}` is not installed or not in $PATH")
        }
        std::io::ErrorKind::NotFound => {
            format!("configured LSP server executable `{program}` was not found")
        }
        _ => format!("failed to execute LSP server `{program}`: {error}"),
    }
}

fn resolve_server(
    detection: &crate::detect::DetectionResult,
    suggestions: &[SuggestedLanguage],
    selected_server: Option<&str>,
) -> Result<SuggestedLanguage, String> {
    let selected = select_server(detection, suggestions, selected_server)?.clone();
    let resolved = resolve_detect_suggestions(std::slice::from_ref(&selected), false)?;
    Ok(resolved.into_iter().next().unwrap_or(selected))
}

#[cfg(test)]
mod tests {
    use super::resolve_server;
    use crate::detect::DetectionResult;
    use crate::mason::registry::{MasonNeovim, MasonPackage, MasonSource};
    use crate::runtime_state::RuntimeState;
    use crate::suggest::SuggestedLanguage;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
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
                "lsp-cli-run-test-{}-{}",
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

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn suggestion(program: &str, config_id: &str, server: &str) -> SuggestedLanguage {
        SuggestedLanguage {
            config_id: config_id.to_string(),
            languages: vec!["python".to_string()],
            server: server.to_string(),
            command: vec![program.to_string(), "--stdio".to_string()],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
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

    fn write_registry(state: &RuntimeState, package: MasonPackage) {
        fs::create_dir_all(state.registry_dir()).expect("registry dir should be created");
        let bytes = serde_json::to_vec(&vec![package]).expect("registry should serialize");
        fs::write(state.registry_json_path(), bytes).expect("registry should be written");
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        let mut permissions = fs::metadata(path)
            .expect("metadata should be available")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("permissions should be updated");
    }

    #[cfg(unix)]
    #[test]
    fn resolves_run_server_from_managed_install() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let dir = TestDir::new();
        let home = dir.path().join("home");
        let state = RuntimeState::new(home.join(".local/share/lsp-cli"));
        state.ensure_dirs().expect("state dirs should be created");
        write_registry(&state, pyright_package());
        let cached = state
            .package_dir("pyright")
            .join("node_modules/.bin/pyright-langserver");
        fs::create_dir_all(cached.parent().expect("parent should exist"))
            .expect("parent dirs should be created");
        fs::write(&cached, b"#!/bin/sh\nexit 0\n").expect("cached binary should be written");
        make_executable(&cached);

        let original_home = std::env::var_os("HOME");
        let original_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("HOME", &home) };
        unsafe { std::env::set_var("PATH", "/nonexistent") };
        let resolved = resolve_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["python".to_string()]),
                filenames: BTreeSet::new(),
            },
            &[suggestion("pyright-langserver", "pyright", "pyright")],
            None,
        )
        .expect("run server should resolve");
        match original_home {
            Some(home) => unsafe { std::env::set_var("HOME", home) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert_eq!(resolved.command[0], cached.display().to_string());
    }
}
