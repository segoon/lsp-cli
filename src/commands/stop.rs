use crate::cli::{StopAllArgs, StopArgs};
use crate::commands::common::{analyze_path, prepare_workspace, resolve_server};
use crate::commands::daemon::{StopSocketResult, stop_socket};
use crate::config::ConfigStore;
use crate::detect::DetectionResult;
use crate::error::{Error, Result};
use crate::runtime_state::{daemon_socket_paths, default_daemon_root};
use crate::suggest::SuggestedLanguage;
use std::collections::BTreeSet;

pub(super) fn run(args: &StopArgs, config: &ConfigStore) -> Result<String> {
    if args.selector.lang.is_none() && args.selector.lsp.is_none() {
        let (detection, suggestions) = analyze_path(&args.path, config)?;
        let languages = implicit_stop_languages(&detection, &suggestions, config);
        if languages.len() > 1 {
            return run_for_languages(args, config, &languages);
        }
    }

    let workspace = prepare_workspace(
        &args.path,
        args.selector.lsp.as_deref(),
        args.selector.lang.as_deref(),
        false,
        config,
    )?;
    let socket_path = workspace.daemon_socket_path.as_ref().ok_or_else(|| {
        let reason = workspace
            .daemon_socket_error
            .as_deref()
            .unwrap_or("daemon socket path could not be prepared for this workspace");
        Error::unexpected(format!("cannot stop daemon because {reason}"))
    })?;

    match stop_socket(socket_path, args.debug)? {
        StopSocketResult::Stopped => Ok(format!(
            "stopped {} daemon for {}",
            workspace.server.server,
            workspace.server.workspace_root.display()
        )),
        StopSocketResult::RemovedStaleSocket | StopSocketResult::NotRunning => Ok(format!(
            "no active {} daemon found for {}",
            workspace.server.server,
            workspace.server.workspace_root.display()
        )),
    }
}

fn run_for_languages(
    args: &StopArgs,
    config: &ConfigStore,
    languages: &[String],
) -> Result<String> {
    let mut stopped = 0usize;
    let mut not_running = 0usize;
    let mut seen_sockets = BTreeSet::new();

    for language in languages {
        let workspace = prepare_workspace(&args.path, None, Some(language), false, config)?;
        let socket_path = workspace.daemon_socket_path.as_ref().ok_or_else(|| {
            let reason = workspace
                .daemon_socket_error
                .as_deref()
                .unwrap_or("daemon socket path could not be prepared for this workspace");
            Error::unexpected(format!("cannot stop daemon because {reason}"))
        })?;
        let socket_key = socket_path.display().to_string();
        if seen_sockets.contains(&socket_key) {
            continue;
        }

        match stop_socket(socket_path, args.debug)? {
            StopSocketResult::Stopped => stopped += 1,
            StopSocketResult::RemovedStaleSocket | StopSocketResult::NotRunning => {
                not_running += 1;
            }
        }
        seen_sockets.insert(socket_key);
    }

    Ok(match (stopped, not_running) {
        (0, 0) => "no active matching daemons found".to_string(),
        (0, 1) => "no active matching daemon found".to_string(),
        (0, not_running) => format!("no active matching daemons found for {not_running} targets"),
        (1, 0) => "stopped 1 matching daemon".to_string(),
        (stopped, 0) => format!("stopped {stopped} matching daemons"),
        (1, 1) => "stopped 1 matching daemon; 1 matching daemon was not active".to_string(),
        (1, not_running) => {
            format!("stopped 1 matching daemon; {not_running} matching daemons were not active")
        }
        (stopped, 1) => {
            format!("stopped {stopped} matching daemons; 1 matching daemon was not active")
        }
        (stopped, not_running) => format!(
            "stopped {stopped} matching daemons; {not_running} matching daemons were not active"
        ),
    })
}

fn implicit_stop_languages(
    detection: &DetectionResult,
    suggestions: &[SuggestedLanguage],
    config: &ConfigStore,
) -> Vec<String> {
    detection
        .filetypes
        .iter()
        .filter(|language| {
            resolve_server(
                detection,
                suggestions,
                None,
                Some(language.as_str()),
                &config.cli.lsp_preferences,
                false,
            )
            .is_ok()
        })
        .cloned()
        .collect()
}

pub(super) fn run_all(args: &StopAllArgs) -> Result<String> {
    let daemon_root = default_daemon_root()?;
    let socket_paths = daemon_socket_paths(&daemon_root)?;
    let mut stopped = 0usize;
    let mut removed_stale = 0usize;
    let mut failures = Vec::new();

    for socket_path in socket_paths {
        match stop_socket(&socket_path, args.debug) {
            Ok(StopSocketResult::Stopped) => stopped += 1,
            Ok(StopSocketResult::RemovedStaleSocket) => removed_stale += 1,
            Ok(StopSocketResult::NotRunning) => {}
            Err(error) => failures.push(format!("{}: {error}", socket_path.display())),
        }
    }

    if !failures.is_empty() {
        return Err(Error::unexpected(format!(
            "failed to stop some daemons:\n{}",
            failures.join("\n")
        )));
    }

    Ok(match (stopped, removed_stale) {
        (0, 0) => "no active daemons found".to_string(),
        (stopped, 0) => format!("stopped {stopped} daemon{}", plural_suffix(stopped)),
        (0, removed_stale) => format!(
            "removed {removed_stale} stale daemon socket{}",
            plural_suffix(removed_stale)
        ),
        (stopped, removed_stale) => format!(
            "stopped {stopped} daemon{} and removed {removed_stale} stale socket{}",
            plural_suffix(stopped),
            plural_suffix(removed_stale)
        ),
    })
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::{implicit_stop_languages, plural_suffix, run_all};
    use crate::cli::StopAllArgs;
    use crate::config::{CliConfig, ConfigStore};
    use crate::test_support::{
        TestDir, current_test_executable, detection_result, env_var, suggested_language,
        with_env_vars,
    };
    use std::fs;

    #[test]
    fn plural_suffix_tracks_plural_forms() {
        assert_eq!(plural_suffix(1), "");
        assert_eq!(plural_suffix(2), "s");
    }

    #[test]
    fn stop_all_reports_no_active_daemons_when_runtime_dir_is_empty() {
        let dir = TestDir::new("stop-all-empty");
        let runtime_dir = dir.path().join("runtime");
        fs::create_dir_all(runtime_dir.join("lsp-cli")).expect("runtime dir should exist");

        let output = with_env_vars(&[env_var("XDG_RUNTIME_DIR", &runtime_dir)], || {
            run_all(&StopAllArgs { debug: false }).expect("stop-all should succeed")
        });

        assert_eq!(output, "no active daemons found");
    }

    #[test]
    fn implicit_stop_languages_include_each_runnable_detected_language() {
        let config = ConfigStore {
            filetypes: Vec::new(),
            lsps: Vec::new(),
            cli: CliConfig::default(),
        };
        let detection = detection_result(&["alpha", "beta"], &[]);
        let executable = current_test_executable().display().to_string();
        let suggestions = vec![
            suggested_language(&executable, "alpha-lsp", "alpha-lsp", "alpha"),
            suggested_language(&executable, "beta-lsp", "beta-lsp", "beta"),
        ];

        assert_eq!(
            implicit_stop_languages(&detection, &suggestions, &config),
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }
}
