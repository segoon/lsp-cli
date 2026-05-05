use crate::error::{Error, Result};
use crate::mason::install::{resolve_cached_program, resolve_or_install_program};
use crate::mason::link::{is_command_runnable, rewrite_program};
use crate::mason::registry::MasonRegistry;
use crate::runtime_state::{RuntimeState, default_runtime_state_root};
use crate::suggest::SuggestedLanguage;

pub fn resolve_detect_suggestions(
    suggestions: &[SuggestedLanguage],
    download: bool,
) -> Result<Vec<SuggestedLanguage>> {
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

    match errors.len() {
        0 => Ok(Vec::new()),
        1 => Err(errors.remove(0)),
        _ => Err(Error::unexpected(
            errors
                .into_iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )),
    }
}

fn resolve_suggestion_from_path_or_cache(
    suggestion: &SuggestedLanguage,
    state: Option<&RuntimeState>,
    registry: Option<&MasonRegistry>,
) -> Result<Option<SuggestedLanguage>> {
    let Some(program) = suggestion.command.first() else {
        return Err(Error::unexpected(format!(
            "selected LSP server {} has an empty command",
            suggestion.server
        )));
    };

    if is_command_runnable(program) {
        return Ok(Some(suggestion.clone()));
    }

    if program.contains(std::path::MAIN_SEPARATOR) {
        return Ok(None);
    }

    let Some(state) = state else {
        return Ok(None);
    };
    let Some(registry) = registry else {
        return Ok(None);
    };
    let Some(package) =
        registry.package_for_detected(&suggestion.config_id, &suggestion.server, program)
    else {
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
) -> Result<SuggestedLanguage> {
    let Some(program) = suggestion.command.first() else {
        return Err(Error::unexpected(format!(
            "selected LSP server {} has an empty command",
            suggestion.server
        )));
    };
    if program.contains(std::path::MAIN_SEPARATOR) {
        return Err(Error::missing_executable(format!(
            "configured LSP server executable `{program}` was not found"
        )));
    }

    let state = state.ok_or_else(|| {
        Error::unexpected("cannot install LSP servers automatically because $HOME is not set")
    })?;
    let registry = registry.get_or_insert(MasonRegistry::load(state)?);
    let package = registry
        .package_for_detected(&suggestion.config_id, &suggestion.server, program)
        .ok_or_else(|| {
            Error::unexpected(format!(
                "no Mason install recipe is available for detected server {}",
                suggestion.server
            ))
        })?;
    let executable_path = resolve_or_install_program(state, package, program)?;

    Ok(rewrite_program(suggestion, &executable_path))
}

#[cfg(test)]
mod tests {
    use super::resolve_detect_suggestions;
    use crate::test_support::{
        TestDir, env_var, jdtls_package, make_executable, pyright_package, runtime_state_in_home,
        suggested_language, with_env_vars, write_registry,
    };
    use std::fs;

    fn prepare_registry_test_home(
        package_name: &str,
        packages: &[crate::mason::registry::MasonPackage],
    ) -> (
        TestDir,
        std::path::PathBuf,
        crate::runtime_state::RuntimeState,
    ) {
        let dir = TestDir::new("mason-resolve");
        let home = dir.path().join("home");
        let state = runtime_state_in_home(&home);
        state.ensure_dirs().expect("state dirs should be created");
        write_registry(&state, packages);
        let package_dir = state.package_dir(package_name);
        (dir, package_dir, state)
    }

    #[cfg(unix)]
    #[test]
    fn prefers_cached_direct_binary_when_path_misses() {
        let (dir, package_dir, _state) =
            prepare_registry_test_home("pyright", &[pyright_package()]);
        let home = dir.path().join("home");
        let cached = package_dir.join("node_modules/.bin/pyright-langserver");
        fs::create_dir_all(cached.parent().expect("parent should exist"))
            .expect("parent dirs should be created");
        fs::write(&cached, b"stub\n").expect("cached binary should be written");
        make_executable(&cached);

        let resolved = with_env_vars(
            &[env_var("HOME", &home), env_var("PATH", "/nonexistent")],
            || {
                resolve_detect_suggestions(
                    &[suggested_language(
                        "pyright-langserver",
                        "pyright",
                        "pyright",
                        "python",
                    )],
                    false,
                )
                .expect("resolution should succeed")
            },
        );

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].command[0], cached.display().to_string());
    }

    #[cfg(unix)]
    #[test]
    fn prefers_cached_wrapper_when_path_misses() {
        let (dir, package_dir, state) = prepare_registry_test_home("jdtls", &[jdtls_package()]);
        let home = dir.path().join("home");
        let target = package_dir.join("bin/jdtls");
        fs::create_dir_all(target.parent().expect("parent should exist"))
            .expect("parent dirs should be created");
        fs::write(&target, b"print('ok')\n").expect("target should be written");
        let launcher = state.bin_dir().join("jdtls");
        fs::write(&launcher, b"stub\n").expect("launcher should be written");
        make_executable(&launcher);
        let runtime_dir = dir.path().join("bin");
        fs::create_dir_all(&runtime_dir).expect("runtime dir should be created");
        let python = runtime_dir.join("python3");
        fs::write(&python, b"stub\n").expect("runtime should be written");
        make_executable(&python);

        let resolved = with_env_vars(
            &[
                env_var("HOME", &home),
                env_var("PATH", runtime_dir.display().to_string()),
            ],
            || {
                resolve_detect_suggestions(
                    &[suggested_language("jdtls", "jdtls", "jdtls", "python")],
                    false,
                )
                .expect("resolution should succeed")
            },
        );

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].command[0], launcher.display().to_string());
    }

    #[cfg(unix)]
    #[test]
    fn skips_server_when_not_in_path_or_cache() {
        let dir = TestDir::new("mason-resolve");
        let home = dir.path().join("home");
        fs::create_dir_all(&home).expect("home dir should be created");

        let resolved = with_env_vars(
            &[env_var("HOME", &home), env_var("PATH", "/nonexistent")],
            || {
                resolve_detect_suggestions(
                    &[suggested_language(
                        "pyright-langserver",
                        "pyright",
                        "pyright",
                        "python",
                    )],
                    false,
                )
                .expect("resolution should succeed")
            },
        );

        assert!(resolved.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn treats_corrupted_cache_as_missing() {
        let dir = TestDir::new("mason-resolve");
        let home = dir.path().join("home");
        let state = runtime_state_in_home(&home);
        state.ensure_dirs().expect("state dirs should be created");
        fs::write(state.registry_json_path(), b"not json")
            .expect("corrupted registry should be written");

        let resolved = with_env_vars(
            &[env_var("HOME", &home), env_var("PATH", "/nonexistent")],
            || {
                resolve_detect_suggestions(
                    &[suggested_language(
                        "pyright-langserver",
                        "pyright",
                        "pyright",
                        "python",
                    )],
                    false,
                )
                .expect("resolution should succeed")
            },
        );

        assert!(resolved.is_empty());
    }
}
