use crate::mason::install::{resolve_cached_program, resolve_or_install_program};
use crate::mason::link::{is_command_runnable, rewrite_program};
use crate::mason::registry::MasonRegistry;
use crate::runtime_state::{RuntimeState, default_runtime_state_root};
use crate::suggest::SuggestedLanguage;

pub fn resolve_detect_suggestions(
    suggestions: &[SuggestedLanguage],
    download: bool,
) -> Result<Vec<SuggestedLanguage>, String> {
    let state = default_runtime_state_root().ok().map(RuntimeState::new);
    let cached_registry = state.as_ref().and_then(MasonRegistry::load_cached);
    let mut install_registry = None;
    let mut resolved = Vec::new();
    let mut errors = Vec::new();

    for suggestion in suggestions {
        if let Some(suggestion) = resolve_suggestion_from_path_or_cache(
            suggestion,
            state.as_ref(),
            cached_registry.as_ref(),
        )? {
            resolved.push(suggestion);
            continue;
        }

        if !download {
            continue;
        }

        match install_suggestion(suggestion, state.as_ref(), &mut install_registry) {
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
    } else if errors.is_empty() {
        Ok(Vec::new())
    } else {
        Err(errors.join("\n"))
    }
}

fn resolve_suggestion_from_path_or_cache(
    suggestion: &SuggestedLanguage,
    state: Option<&RuntimeState>,
    registry: Option<&MasonRegistry>,
) -> Result<Option<SuggestedLanguage>, String> {
    let Some(program) = suggestion.command.first() else {
        return Err(format!(
            "selected LSP server {} has an empty command",
            suggestion.server
        ));
    };

    if is_command_runnable(program) {
        return Ok(Some(suggestion.clone()));
    }

    if program.contains(std::path::MAIN_SEPARATOR) {
        return Ok(None);
    }

    let (Some(state), Some(registry)) = (state, registry) else {
        return Ok(None);
    };
    let Some(package) = registry.package_for_detected(&suggestion.config_id, &suggestion.server, program) else {
        return Ok(None);
    };

    let Ok(Some(executable_path)) = resolve_cached_program(state, package, program) else {
        return Ok(None);
    };

    Ok(Some(rewrite_program(suggestion, &executable_path)))
}

fn install_suggestion(
    suggestion: &SuggestedLanguage,
    state: Option<&RuntimeState>,
    registry: &mut Option<MasonRegistry>,
) -> Result<SuggestedLanguage, String> {
    let Some(program) = suggestion.command.first() else {
        return Err(format!(
            "selected LSP server {} has an empty command",
            suggestion.server
        ));
    };
    if program.contains(std::path::MAIN_SEPARATOR) {
        return Err(format!(
            "configured LSP server executable `{program}` was not found"
        ));
    }

    let state = state
        .ok_or_else(|| "cannot install LSP servers automatically because $HOME is not set".to_string())?;
    let registry = registry.get_or_insert(MasonRegistry::load(state)?);
    let package = registry
        .package_for_detected(&suggestion.config_id, &suggestion.server, program)
        .ok_or_else(|| {
            format!(
                "no Mason install recipe is available for detected server {}",
                suggestion.server
            )
        })?;
    let executable_path = resolve_or_install_program(state, package, program)?;

    Ok(rewrite_program(suggestion, &executable_path))
}

#[cfg(test)]
mod tests {
    use super::resolve_detect_suggestions;
    use crate::mason::registry::{MasonNeovim, MasonPackage, MasonSource};
    use crate::runtime_state::RuntimeState;
    use crate::suggest::SuggestedLanguage;
    use std::collections::BTreeMap;
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

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn write_registry(state: &RuntimeState, package: MasonPackage) {
        fs::create_dir_all(state.registry_dir()).expect("registry dir should be created");
        let bytes = serde_json::to_vec(&vec![package]).expect("registry should serialize");
        fs::write(state.registry_json_path(), bytes).expect("registry should be written");
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

    fn jdtls_package() -> MasonPackage {
        MasonPackage {
            name: "jdtls".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:generic/eclipse/eclipse.jdt.ls@v1.0.0".to_string(),
                extra_packages: Vec::new(),
                asset: None,
                download: Some(crate::mason::registry::OneOrMany::Many(vec![
                    crate::mason::registry::MasonDownload {
                        target: Some(crate::mason::registry::OneOrMany::One("linux".to_string())),
                        files: BTreeMap::from([(
                            "jdtls.tar.gz".to_string(),
                            "https://example.invalid/jdtls.tar.gz".to_string(),
                        )]),
                        bin: None,
                        config: Some("config_linux/".to_string()),
                        man: None,
                    },
                ])),
                version_overrides: Vec::new(),
            },
            bin: BTreeMap::from([("jdtls".to_string(), "python:bin/jdtls".to_string())]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("jdtls".to_string()),
            },
        }
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
    fn prefers_cached_direct_binary_when_path_misses() {
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
        let resolved = resolve_detect_suggestions(&[suggestion("pyright-langserver", "pyright", "pyright")], false)
            .expect("resolution should succeed");
        match original_home {
            Some(home) => unsafe { std::env::set_var("HOME", home) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].command[0], cached.display().to_string());
    }

    #[cfg(unix)]
    #[test]
    fn prefers_cached_wrapper_when_path_misses() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let dir = TestDir::new();
        let home = dir.path().join("home");
        let state = RuntimeState::new(home.join(".local/share/lsp-cli"));
        state.ensure_dirs().expect("state dirs should be created");
        write_registry(&state, jdtls_package());
        let target = state.package_dir("jdtls").join("bin/jdtls");
        fs::create_dir_all(target.parent().expect("parent should exist"))
            .expect("parent dirs should be created");
        fs::write(&target, b"print('ok')\n").expect("target should be written");
        let launcher = state.bin_dir().join("jdtls");
        fs::write(&launcher, b"#!/bin/sh\nexec python3 /tmp/fake \"$@\"\n")
            .expect("launcher should be written");
        make_executable(&launcher);

        let original_home = std::env::var_os("HOME");
        let original_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("HOME", &home) };
        unsafe { std::env::set_var("PATH", "/usr/bin") };
        let resolved = resolve_detect_suggestions(&[suggestion("jdtls", "jdtls", "jdtls")], false)
            .expect("resolution should succeed");
        match original_home {
            Some(home) => unsafe { std::env::set_var("HOME", home) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].command[0], launcher.display().to_string());
    }

    #[cfg(unix)]
    #[test]
    fn skips_server_when_not_in_path_or_cache() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let dir = TestDir::new();
        let home = dir.path().join("home");
        fs::create_dir_all(&home).expect("home dir should be created");

        let original_home = std::env::var_os("HOME");
        let original_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("HOME", &home) };
        unsafe { std::env::set_var("PATH", "/nonexistent") };
        let resolved = resolve_detect_suggestions(&[suggestion("pyright-langserver", "pyright", "pyright")], false)
            .expect("resolution should succeed");
        match original_home {
            Some(home) => unsafe { std::env::set_var("HOME", home) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert!(resolved.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn treats_corrupted_cache_as_missing() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let dir = TestDir::new();
        let home = dir.path().join("home");
        let state = RuntimeState::new(home.join(".local/share/lsp-cli"));
        state.ensure_dirs().expect("state dirs should be created");
        fs::write(state.registry_json_path(), b"not json").expect("corrupted registry should be written");

        let original_home = std::env::var_os("HOME");
        let original_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("HOME", &home) };
        unsafe { std::env::set_var("PATH", "/nonexistent") };
        let resolved = resolve_detect_suggestions(&[suggestion("pyright-langserver", "pyright", "pyright")], false)
            .expect("resolution should succeed");
        match original_home {
            Some(home) => unsafe { std::env::set_var("HOME", home) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert!(resolved.is_empty());
    }
}
