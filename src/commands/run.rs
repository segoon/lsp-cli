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
    use crate::test_support::{
        TestDir, env_var, make_executable, pyright_package, runtime_state_in_home,
        suggested_language, with_env_vars, write_registry,
    };
    use std::collections::BTreeSet;
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn resolves_run_server_from_managed_install() {
        let dir = TestDir::new("run");
        let home = dir.path().join("home");
        let state = runtime_state_in_home(&home);
        state.ensure_dirs().expect("state dirs should be created");
        write_registry(&state, &[pyright_package()]);
        let cached = state
            .package_dir("pyright")
            .join("node_modules/.bin/pyright-langserver");
        fs::create_dir_all(cached.parent().expect("parent should exist"))
            .expect("parent dirs should be created");
        fs::write(&cached, b"#!/bin/sh\nexit 0\n").expect("cached binary should be written");
        make_executable(&cached);

        let resolved = with_env_vars(
            &[env_var("HOME", &home), env_var("PATH", "/nonexistent")],
            || {
                resolve_server(
                    &DetectionResult {
                        filetypes: BTreeSet::from(["python".to_string()]),
                        filenames: BTreeSet::new(),
                    },
                    &[suggested_language(
                        "pyright-langserver",
                        "pyright",
                        "pyright",
                        "python",
                    )],
                    None,
                )
                .expect("run server should resolve")
            },
        );

        assert_eq!(resolved.command[0], cached.display().to_string());
    }
}
