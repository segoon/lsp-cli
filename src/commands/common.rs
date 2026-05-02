use crate::commands::daemon::launch_for_workspace;
use crate::config::ConfigStore;
use crate::detect::{DetectionResult, detect_workspace};
use crate::lsp::path_to_file_uri;
use crate::mason::resolve_detect_suggestions;
use crate::runtime_state::{daemon_socket_path, default_daemon_root};
use crate::suggest::{SuggestedLanguage, sort_suggestions, suggestions_for};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use crate::lsp::LspClient;

pub(super) struct PreparedWorkspace {
    pub detection: DetectionResult,
    pub server: SuggestedLanguage,
    pub allowed_filetypes: BTreeSet<String>,
    pub root_uri: String,
    pub workspace_name: String,
    pub daemon_socket_path: Option<PathBuf>,
    pub daemon_socket_error: Option<String>,
}

#[derive(Debug)]
pub(super) struct ResolvedServer {
    pub server: SuggestedLanguage,
    pub allowed_filetypes: BTreeSet<String>,
}

pub(super) fn analyze_path(
    path: &Path,
    config: &ConfigStore,
) -> Result<(DetectionResult, Vec<SuggestedLanguage>), String> {
    let detection = detect_workspace(path, &config.filetypes)
        .map_err(|error| format!("failed to scan {}: {error}", path.display()))?;
    let mut suggestions = suggestions_for(&config.lsps, &detection, path)
        .map_err(|error| format!("failed to build suggestions: {error}"))?;
    sort_suggestions(&mut suggestions, &config.cli.lsp_preferences, None);

    Ok((detection, suggestions))
}

pub(super) fn prepare_workspace(
    path: &Path,
    selected_server: Option<&str>,
    selected_language: Option<&str>,
    download: bool,
    config: &ConfigStore,
) -> Result<PreparedWorkspace, String> {
    let (detection, suggestions) = analyze_path(path, config)?;
    let resolved = resolve_server(
        &detection,
        &suggestions,
        selected_server,
        selected_language,
        &config.cli.lsp_preferences,
        download,
    )?;
    let mut server = resolved.server;
    server.workspace_root = fs::canonicalize(&server.workspace_root).map_err(|error| {
        format!(
            "failed to resolve {}: {error}",
            server.workspace_root.display()
        )
    })?;
    let root_uri = path_to_file_uri(&server.workspace_root)?;
    let workspace_name = crate::lsp::workspace_name(&server.workspace_root);
    let (daemon_socket_path, daemon_socket_error) = match default_daemon_root() {
        Ok(daemon_root) => (
            Some(daemon_socket_path(
                &daemon_root,
                &server.workspace_root,
                &server.server,
                &server.command,
            )),
            None,
        ),
        Err(error) => (None, Some(error)),
    };

    Ok(PreparedWorkspace {
        detection,
        server,
        allowed_filetypes: resolved.allowed_filetypes,
        root_uri,
        workspace_name,
        daemon_socket_path,
        daemon_socket_error,
    })
}

pub(super) fn connect_lsp_client(
    workspace: &PreparedWorkspace,
    detach: bool,
    debug: bool,
    timeout: Duration,
) -> Result<LspClient, String> {
    if let Some(socket_path) = workspace.daemon_socket_path.as_ref()
        && socket_path.exists()
    {
        match LspClient::connect_unix(socket_path, debug, timeout) {
            Ok(client) => return Ok(client),
            Err(connect_error) => {
                match fs::remove_file(socket_path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!(
                            "failed to clean up dead daemon socket {}: {error}",
                            socket_path.display()
                        ));
                    }
                }

                if !detach {
                    return LspClient::new(
                        &workspace.server.command,
                        &workspace.server.workspace_root,
                        debug,
                        timeout,
                    )
                    .map_err(|spawn_error| {
                        format!(
                            "failed to use daemon socket {}: {connect_error}; failed to start {}: {spawn_error}",
                            socket_path.display(),
                            workspace.server.server
                        )
                    });
                }
            }
        }
    }

    if detach {
        let socket_path = workspace.daemon_socket_path.as_ref().ok_or_else(|| {
            let reason = workspace
                .daemon_socket_error
                .as_deref()
                .unwrap_or("daemon socket path could not be prepared for this workspace");
            format!("cannot use --detach because {reason}")
        })?;
        launch_for_workspace(
            &workspace.server.workspace_root,
            &workspace.server.server,
            socket_path,
            debug,
        )?;
        return LspClient::connect_unix(socket_path, debug, timeout).map_err(|error| {
            format!(
                "failed to connect to detached daemon for {}: {error}",
                workspace.server.server
            )
        });
    }

    LspClient::new(
        &workspace.server.command,
        &workspace.server.workspace_root,
        debug,
        timeout,
    )
}

pub(super) fn resolve_server(
    detection: &DetectionResult,
    suggestions: &[SuggestedLanguage],
    selected_server: Option<&str>,
    selected_language: Option<&str>,
    lsp_preferences: &std::collections::BTreeMap<String, Vec<String>>,
    download: bool,
) -> Result<ResolvedServer, String> {
    let mut candidates = selection_candidates(suggestions, download)?;

    if let Some(server) = selected_server {
        return resolve_explicit_server(
            suggestions,
            &mut candidates,
            server,
            selected_language,
            lsp_preferences,
            download,
        );
    }

    if let Some(language) = selected_language {
        candidates.retain(|suggestion| suggestion.languages.iter().any(|value| value == language));
        if candidates.is_empty() {
            if suggestions
                .iter()
                .any(|suggestion| suggestion.languages.iter().any(|value| value == language))
            {
                return Err(no_runnable_server_for_language_error(language));
            }
            return Err(format!(
                "no LSP server was detected for language {language:?}"
            ));
        }
        sort_suggestions(&mut candidates, lsp_preferences, Some(language));
        return resolve_candidate(candidates[0].clone(), Some(language), download);
    }

    let languages = detected_languages(&candidates);
    let language_names = languages.iter().cloned().collect::<Vec<_>>();
    if language_names.len() > 1 {
        return Err(format!(
            "multiple languages were detected for this command: {}; pass --lang LANG or --lsp SERVER to choose one",
            language_names.join(", ")
        ));
    }

    let Some(language) = language_names.into_iter().next() else {
        return Err(no_resolved_server_error(detection, download));
    };

    sort_suggestions(&mut candidates, lsp_preferences, Some(&language));
    resolve_candidate(candidates[0].clone(), Some(&language), download)
}

fn selection_candidates(
    suggestions: &[SuggestedLanguage],
    download: bool,
) -> Result<Vec<SuggestedLanguage>, String> {
    if download {
        Ok(suggestions.to_vec())
    } else {
        resolve_detect_suggestions(suggestions, false)
    }
}

fn resolve_explicit_server(
    suggestions: &[SuggestedLanguage],
    candidates: &mut Vec<SuggestedLanguage>,
    selected_server: &str,
    selected_language: Option<&str>,
    lsp_preferences: &std::collections::BTreeMap<String, Vec<String>>,
    download: bool,
) -> Result<ResolvedServer, String> {
    let mut detected_candidates = suggestions
        .iter()
        .filter(|suggestion| suggestion.server == selected_server)
        .cloned()
        .collect::<Vec<_>>();
    candidates.retain(|suggestion| suggestion.server == selected_server);

    if let Some(language) = selected_language {
        detected_candidates
            .retain(|suggestion| suggestion.languages.iter().any(|value| value == language));
        candidates.retain(|suggestion| suggestion.languages.iter().any(|value| value == language));
        if detected_candidates.is_empty() {
            return Err(format!(
                "requested LSP server {selected_server:?} is not available for language {language:?}"
            ));
        }
        if candidates.is_empty() {
            return Err(explicit_server_not_runnable_error(selected_server, Some(language)));
        }
        sort_suggestions(candidates, lsp_preferences, Some(language));
        return resolve_candidate(candidates[0].clone(), Some(language), download);
    }

    if detected_candidates.is_empty() {
        let available = suggestions
            .iter()
            .map(|suggestion| suggestion.server.as_str())
            .collect::<Vec<_>>();
        return Err(if available.is_empty() {
            format!(
                "requested LSP server {selected_server:?} is not available because no matching servers were detected"
            )
        } else {
            format!(
                "requested LSP server {selected_server:?} is not in the detected server list: {}",
                available.join(", ")
            )
        });
    }

    if candidates.is_empty() {
        return Err(explicit_server_not_runnable_error(selected_server, None));
    }

    resolve_candidate(candidates[0].clone(), None, download)
}

fn resolve_candidate(
    selected: SuggestedLanguage,
    language: Option<&str>,
    download: bool,
) -> Result<ResolvedServer, String> {
    let server = if download {
        resolve_detect_suggestions(std::slice::from_ref(&selected), true)?
            .into_iter()
            .next()
            .unwrap_or(selected)
    } else {
        selected
    };
    let allowed_filetypes = match language {
        Some(language) => BTreeSet::from([language.to_string()]),
        None => server.languages.iter().cloned().collect(),
    };

    Ok(ResolvedServer {
        server,
        allowed_filetypes,
    })
}

fn explicit_server_not_runnable_error(selected_server: &str, language: Option<&str>) -> String {
    match language {
        Some(language) => format!(
            "requested LSP server {selected_server:?} is not runnable for language {language:?}"
        ),
        None => format!("requested LSP server {selected_server:?} is not runnable"),
    }
}

fn no_runnable_server_for_language_error(language: &str) -> String {
    format!("no runnable LSP server was found for language {language:?}")
}

fn no_resolved_server_error(detection: &DetectionResult, download: bool) -> String {
    if download {
        no_detected_server_error(detection)
    } else if detection.filetypes.is_empty() {
        "No supported languages detected".to_string()
    } else {
        format!(
            "No runnable LSP server found for detected filetypes: {}",
            detection
                .filetypes
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn detected_languages(suggestions: &[SuggestedLanguage]) -> BTreeSet<String> {
    suggestions
        .iter()
        .flat_map(|suggestion| suggestion.languages.iter().cloned())
        .collect()
}

fn no_detected_server_error(detection: &DetectionResult) -> String {
    if detection.filetypes.is_empty() {
        "No supported languages detected".to_string()
    } else {
        format!(
            "No LSP server matches detected filetypes: {}",
            detection
                .filetypes
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{PreparedWorkspace, connect_lsp_client, prepare_workspace, resolve_server};
    use crate::config::load_config_store;
    use crate::detect::DetectionResult;
    use crate::lsp::transport::{read_message, write_message};
    use crate::suggest::SuggestedLanguage;
    use crate::test_support::{
        TestDir, env_var, make_executable, pyright_package, runtime_state_in_home, with_env_vars,
        without_env_vars, write_registry,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::io::BufReader;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    fn example_suggestion() -> SuggestedLanguage {
        SuggestedLanguage {
            config_id: "example_lsp".to_string(),
            languages: vec!["alpha".to_string(), "beta".to_string()],
            server: "example-lsp".to_string(),
            command: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "exit 0".to_string(),
            ],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
        }
    }

    #[test]
    fn selects_requested_server_for_grep() {
        let primary = example_suggestion();
        let secondary = SuggestedLanguage {
            config_id: "secondary_lsp".to_string(),
            languages: vec!["beta".to_string()],
            server: "secondary-lsp".to_string(),
            command: vec!["/bin/true".to_string()],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
        };
        let suggestions = [primary, secondary.clone()];

        let selected = resolve_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            &suggestions,
            Some("secondary-lsp"),
            None,
            &BTreeMap::new(),
            false,
        )
        .expect("requested server should be selected");

        assert_eq!(selected.server.server, secondary.server);
    }

    #[test]
    fn errors_when_requested_server_is_not_detected() {
        let error = resolve_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            &[example_suggestion()],
            Some("missing-lsp"),
            None,
            &BTreeMap::new(),
            false,
        )
        .expect_err("missing server should error");

        assert_eq!(
            error,
            "requested LSP server \"missing-lsp\" is not in the detected server list: example-lsp"
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolves_server_from_managed_install() {
        let dir = crate::test_support::TestDir::new("common");
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
                    &[SuggestedLanguage {
                        config_id: "pyright".to_string(),
                        languages: vec!["python".to_string()],
                        server: "pyright-langserver".to_string(),
                        command: vec!["pyright-langserver".to_string(), "--stdio".to_string()],
                        workspace_root: PathBuf::from("."),
                        wait_for_index: false,
                    }],
                    None,
                    None,
                    &BTreeMap::new(),
                    false,
                )
                .expect("server should resolve")
            },
        );

        assert_eq!(resolved.server.command[0], cached.display().to_string());
    }

    #[test]
    fn errors_when_auto_selection_spans_multiple_languages() {
        let error = resolve_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            &[example_suggestion()],
            None,
            None,
            &BTreeMap::new(),
            false,
        )
        .expect_err("multiple languages should require disambiguation");

        assert_eq!(
            error,
            "multiple languages were detected for this command: alpha, beta; pass --lang LANG or --lsp SERVER to choose one"
        );
    }

    #[test]
    fn allows_auto_selection_with_explicit_language() {
        let resolved = resolve_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            &[example_suggestion()],
            None,
            Some("beta"),
            &BTreeMap::new(),
            false,
        )
        .expect("language should disambiguate");
        assert_eq!(
            resolved.allowed_filetypes,
            BTreeSet::from(["beta".to_string()])
        );
    }

    #[cfg(unix)]
    #[test]
    fn skips_unrunnable_servers_without_download() {
        let dir = TestDir::new("common-select-installed");
        let home = dir.path().join("home");
        fs::create_dir_all(&home).expect("home dir should be created");
        let bin_dir = dir.path().join("bin");
        fs::create_dir_all(&bin_dir).expect("bin dir should be created");
        let fallback = bin_dir.join("fallback-lsp");
        fs::write(&fallback, b"#!/bin/sh\nexit 0\n").expect("fallback server should be written");
        make_executable(&fallback);

        let resolved = with_env_vars(
            &[
                env_var("HOME", &home),
                env_var("PATH", bin_dir.display().to_string()),
            ],
            || {
                resolve_server(
                    &DetectionResult {
                        filetypes: BTreeSet::from(["python".to_string()]),
                        filenames: BTreeSet::new(),
                    },
                    &[
                        SuggestedLanguage {
                            config_id: "pyright".to_string(),
                            languages: vec!["python".to_string()],
                            server: "pyright".to_string(),
                            command: vec![
                                "pyright-langserver".to_string(),
                                "--stdio".to_string(),
                            ],
                            workspace_root: PathBuf::from("."),
                            wait_for_index: false,
                        },
                        SuggestedLanguage {
                            config_id: "fallback".to_string(),
                            languages: vec!["python".to_string()],
                            server: "fallback-lsp".to_string(),
                            command: vec!["fallback-lsp".to_string(), "--stdio".to_string()],
                            workspace_root: PathBuf::from("."),
                            wait_for_index: false,
                        },
                    ],
                    None,
                    None,
                    &BTreeMap::from([(
                        "python".to_string(),
                        vec!["pyright".to_string(), "fallback-lsp".to_string()],
                    )]),
                    false,
                )
                .expect("fallback server should be selected")
            },
        );

        assert_eq!(resolved.server.server, "fallback-lsp");
        assert_eq!(resolved.server.command[0], "fallback-lsp");
    }

    #[cfg(unix)]
    #[test]
    fn downloads_selected_server_when_requested() {
        let dir = TestDir::new("common-download-selected");
        let home = dir.path().join("home");
        let state = runtime_state_in_home(&home);
        state.ensure_dirs().expect("state dirs should be created");
        write_registry(&state, &[pyright_package()]);

        let bin_dir = dir.path().join("bin");
        fs::create_dir_all(&bin_dir).expect("bin dir should be created");
        let fallback = bin_dir.join("fallback-lsp");
        fs::write(&fallback, b"#!/bin/sh\nexit 0\n").expect("fallback server should be written");
        make_executable(&fallback);
        let npm = bin_dir.join("npm");
        fs::write(
            &npm,
            b"#!/bin/sh\nprefix=\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"--prefix\" ]; then\n    prefix=\"$2\"\n    shift 2\n    continue\n  fi\n  shift\ndone\n/bin/mkdir -p \"$prefix/node_modules/.bin\"\nprintf '#!/bin/sh\\nexit 0\\n' > \"$prefix/node_modules/.bin/pyright-langserver\"\n/bin/chmod 755 \"$prefix/node_modules/.bin/pyright-langserver\"\n",
        )
        .expect("fake npm should be written");
        make_executable(&npm);

        let resolved = with_env_vars(
            &[
                env_var("HOME", &home),
                env_var("PATH", bin_dir.display().to_string()),
            ],
            || {
                resolve_server(
                    &DetectionResult {
                        filetypes: BTreeSet::from(["python".to_string()]),
                        filenames: BTreeSet::new(),
                    },
                    &[
                        SuggestedLanguage {
                            config_id: "pyright".to_string(),
                            languages: vec!["python".to_string()],
                            server: "pyright".to_string(),
                            command: vec![
                                "pyright-langserver".to_string(),
                                "--stdio".to_string(),
                            ],
                            workspace_root: PathBuf::from("."),
                            wait_for_index: false,
                        },
                        SuggestedLanguage {
                            config_id: "fallback".to_string(),
                            languages: vec!["python".to_string()],
                            server: "fallback-lsp".to_string(),
                            command: vec!["fallback-lsp".to_string(), "--stdio".to_string()],
                            workspace_root: PathBuf::from("."),
                            wait_for_index: false,
                        },
                    ],
                    None,
                    None,
                    &BTreeMap::from([(
                        "python".to_string(),
                        vec!["pyright".to_string(), "fallback-lsp".to_string()],
                    )]),
                    true,
                )
                .expect("preferred server should install and resolve")
            },
        );

        let installed = state
            .package_dir("pyright")
            .join("node_modules/.bin/pyright-langserver");
        assert_eq!(resolved.server.server, "pyright");
        assert_eq!(resolved.server.command[0], installed.display().to_string());
        assert!(installed.exists(), "preferred server should be installed");
    }

    #[cfg(unix)]
    #[test]
    fn prefers_live_daemon_socket_before_spawning_server() {
        let dir = TestDir::new("common-daemon");
        let runtime_dir = dir.path().join("runtime");
        fs::create_dir_all(runtime_dir.join("lsp-cli")).expect("runtime dir should be created");
        let socket_path = runtime_dir.join("lsp-cli/test.sock");
        let listener = UnixListener::bind(&socket_path).expect("socket should bind");
        let cwd_file = dir.path().join("cwd.txt");
        let workspace_root = dir.path().join("workspace");
        fs::create_dir_all(&workspace_root).expect("workspace should exist");

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("client should connect");
            let reader_stream = stream.try_clone().expect("socket should clone");
            let mut reader = BufReader::new(reader_stream);
            let mut writer = stream;

            let initialize = read_message(&mut reader)
                .expect("initialize should parse")
                .expect("initialize should exist");
            assert_eq!(
                initialize.get("method").and_then(serde_json::Value::as_str),
                Some("initialize")
            );
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": initialize.get("id").cloned().expect("initialize id should exist"),
                "result": { "capabilities": {} },
            });
            write_message(&mut writer, &response).expect("initialize response should write");

            let initialized = read_message(&mut reader)
                .expect("initialized should parse")
                .expect("initialized should exist");
            assert_eq!(
                initialized
                    .get("method")
                    .and_then(serde_json::Value::as_str),
                Some("initialized")
            );

            let shutdown = read_message(&mut reader)
                .expect("shutdown should parse")
                .expect("shutdown should exist");
            assert_eq!(
                shutdown.get("method").and_then(serde_json::Value::as_str),
                Some("shutdown")
            );
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": shutdown.get("id").cloned().expect("shutdown id should exist"),
                "result": null,
            });
            write_message(&mut writer, &response).expect("shutdown response should write");

            let exit = read_message(&mut reader)
                .expect("exit should parse")
                .expect("exit should exist");
            assert_eq!(
                exit.get("method").and_then(serde_json::Value::as_str),
                Some("exit")
            );
        });

        with_env_vars(&[env_var("XDG_RUNTIME_DIR", &runtime_dir)], || {
            let mut client = connect_lsp_client(
                &PreparedWorkspace {
                    detection: DetectionResult {
                        filetypes: BTreeSet::from(["rust".to_string()]),
                        filenames: BTreeSet::new(),
                    },
                    server: SuggestedLanguage {
                        config_id: "rust-analyzer".to_string(),
                        languages: vec!["rust".to_string()],
                        server: "rust-analyzer".to_string(),
                        command: vec![
                            "/bin/sh".to_string(),
                            "-c".to_string(),
                            format!("pwd > {}", cwd_file.display()),
                        ],
                        workspace_root: workspace_root.clone(),
                        wait_for_index: false,
                    },
                    allowed_filetypes: BTreeSet::from(["rust".to_string()]),
                    root_uri: crate::lsp::path_to_file_uri(&workspace_root)
                        .expect("root uri should build"),
                    workspace_name: crate::lsp::workspace_name(&workspace_root),
                    daemon_socket_path: Some(socket_path.clone()),
                    daemon_socket_error: None,
                },
                false,
                false,
                Duration::from_secs(1),
            )
            .expect("client should connect");

            client
                .initialize(
                    &crate::lsp::path_to_file_uri(&workspace_root).expect("root uri should build"),
                    &crate::lsp::workspace_name(&workspace_root),
                    false,
                )
                .expect("initialize should succeed");
            client.shutdown().expect("shutdown should succeed");
        });

        server.join().expect("daemon thread should finish");
        assert!(
            !cwd_file.exists(),
            "direct server should not have been spawned"
        );
    }

    #[cfg(unix)]
    #[test]
    fn removes_dead_daemon_socket_and_falls_back_to_server_process() {
        let dir = TestDir::new("common-dead-daemon");
        let runtime_dir = dir.path().join("runtime");
        fs::create_dir_all(runtime_dir.join("lsp-cli")).expect("runtime dir should be created");
        let socket_path = runtime_dir.join("lsp-cli/test.sock");
        let listener = UnixListener::bind(&socket_path).expect("socket should bind");
        drop(listener);

        let workspace_root = dir.path().join("workspace");
        fs::create_dir_all(&workspace_root).expect("workspace should exist");
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "exit 0".to_string(),
        ];

        with_env_vars(&[env_var("XDG_RUNTIME_DIR", &runtime_dir)], || {
            let _client = connect_lsp_client(
                &PreparedWorkspace {
                    detection: DetectionResult {
                        filetypes: BTreeSet::from(["rust".to_string()]),
                        filenames: BTreeSet::new(),
                    },
                    server: SuggestedLanguage {
                        config_id: "rust-analyzer".to_string(),
                        languages: vec!["rust".to_string()],
                        server: "rust-analyzer".to_string(),
                        command,
                        workspace_root: workspace_root.clone(),
                        wait_for_index: false,
                    },
                    allowed_filetypes: BTreeSet::from(["rust".to_string()]),
                    root_uri: crate::lsp::path_to_file_uri(&workspace_root)
                        .expect("root uri should build"),
                    workspace_name: crate::lsp::workspace_name(&workspace_root),
                    daemon_socket_path: Some(socket_path.clone()),
                    daemon_socket_error: None,
                },
                false,
                false,
                Duration::from_secs(1),
            )
            .expect("client should fall back to process");
        });

        assert!(!socket_path.exists(), "dead socket should be removed");
    }

    #[test]
    fn preserves_daemon_root_error_for_strict_detach_mode() {
        let dir = TestDir::new("common-daemon-root-error");
        let workspace_root = dir.path().join("workspace");
        fs::create_dir_all(workspace_root.join("src")).expect("workspace should exist");
        fs::write(
            workspace_root.join("Cargo.toml"),
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("cargo manifest should be written");
        fs::write(workspace_root.join("src/main.rs"), b"fn main() {}\n")
            .expect("rust source should be written");

        let config =
            load_config_store(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"))
                .expect("repo config should load");
        let workspace = without_env_vars(&["XDG_RUNTIME_DIR"], || {
            prepare_workspace(&workspace_root, None, None, false, &config)
                .expect("workspace should still prepare")
        });

        assert!(workspace.daemon_socket_path.is_none());
        assert_eq!(
            workspace.daemon_socket_error.as_deref(),
            Some("could not resolve daemon socket root because $XDG_RUNTIME_DIR is not set")
        );
        let error = match connect_lsp_client(&workspace, true, false, Duration::from_secs(1)) {
            Ok(_) => panic!("strict detach should fail"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            "cannot use --detach because could not resolve daemon socket root because $XDG_RUNTIME_DIR is not set"
        );
    }
}
