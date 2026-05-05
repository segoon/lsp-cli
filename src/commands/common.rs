use crate::commands::daemon::launch_for_workspace;
use crate::config::ConfigStore;
use crate::detect::{DetectionResult, detect_workspace};
use crate::error::{Error, Result, error_fn};
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
) -> Result<(DetectionResult, Vec<SuggestedLanguage>)> {
    let detection = detect_workspace(path, &config.filetypes).map_err(error_fn!(
        Error::unexpected,
        "failed to scan {}",
        path.display()
    ))?;
    let mut suggestions = suggestions_for(&config.lsps, &detection, path)?;
    sort_suggestions(&mut suggestions, &config.cli.lsp_preferences, None);

    Ok((detection, suggestions))
}

pub(super) fn prepare_workspace(
    path: &Path,
    selected_server: Option<&str>,
    selected_language: Option<&str>,
    download: bool,
    config: &ConfigStore,
) -> Result<PreparedWorkspace> {
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
    server.workspace_root = fs::canonicalize(&server.workspace_root).map_err(error_fn!(
        Error::unexpected,
        "failed to resolve {}",
        server.workspace_root.display()
    ))?;
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
        Err(error) => (None, Some(error.to_string())),
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
) -> Result<LspClient> {
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
                        return Err(Error::unexpected(format!(
                            "failed to clean up dead daemon socket {}: {error}",
                            socket_path.display()
                        )));
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
                        Error::lsp(format!(
                            "failed to use daemon socket {}: {connect_error}; failed to start {}: {spawn_error}",
                            socket_path.display(),
                            workspace.server.server
                        ))
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
            Error::unexpected(format!("cannot use --detach because {reason}"))
        })?;
        launch_for_workspace(
            &workspace.server.workspace_root,
            &workspace.server.server,
            socket_path,
            debug,
        )?;
        return LspClient::connect_unix(socket_path, debug, timeout).map_err(|error| {
            Error::lsp(format!(
                "failed to connect to detached daemon for {}: {error}",
                workspace.server.server
            ))
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
) -> Result<ResolvedServer> {
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
            return Err(Error::detection(format!(
                "no LSP server was detected for language {language:?}"
            )));
        }
        sort_suggestions(&mut candidates, lsp_preferences, Some(language));
        return resolve_candidate(candidates[0].clone(), Some(language), download);
    }

    let languages = detected_languages(&candidates);
    let language_names = languages.iter().cloned().collect::<Vec<_>>();
    if language_names.len() > 1 {
        return Err(Error::detection(format!(
            "multiple languages were detected for this command: {}; pass --lang LANG or --lsp SERVER to choose one",
            language_names.join(", ")
        )));
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
) -> Result<Vec<SuggestedLanguage>> {
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
) -> Result<ResolvedServer> {
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
            return Err(Error::detection(format!(
                "requested LSP server {selected_server:?} is not available for language {language:?}"
            )));
        }
        if candidates.is_empty() {
            return Err(explicit_server_not_runnable_error(
                selected_server,
                Some(language),
            ));
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
            Error::detection(format!(
                "requested LSP server {selected_server:?} is not available because no matching servers were detected"
            ))
        } else {
            Error::detection(format!(
                "requested LSP server {selected_server:?} is not in the detected server list: {}",
                available.join(", ")
            ))
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
) -> Result<ResolvedServer> {
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

fn explicit_server_not_runnable_error(selected_server: &str, language: Option<&str>) -> Error {
    match language {
        Some(language) => Error::unexpected(format!(
            "requested LSP server {selected_server:?} is not runnable for language {language:?}"
        )),
        None => Error::unexpected(format!(
            "requested LSP server {selected_server:?} is not runnable"
        )),
    }
}

fn no_runnable_server_for_language_error(language: &str) -> Error {
    Error::detection(format!(
        "no runnable LSP server was found for language {language:?}"
    ))
}

fn no_resolved_server_error(detection: &DetectionResult, download: bool) -> Error {
    if download {
        no_detected_server_error(detection)
    } else if detection.filetypes.is_empty() {
        Error::detection("No supported languages detected")
    } else {
        Error::detection(format!(
            "No runnable LSP server found for detected filetypes: {}",
            detection
                .filetypes
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

fn detected_languages(suggestions: &[SuggestedLanguage]) -> BTreeSet<String> {
    suggestions
        .iter()
        .flat_map(|suggestion| suggestion.languages.iter().cloned())
        .collect()
}

fn no_detected_server_error(detection: &DetectionResult) -> Error {
    if detection.filetypes.is_empty() {
        Error::detection("No supported languages detected")
    } else {
        Error::detection(format!(
            "No LSP server matches detected filetypes: {}",
            detection
                .filetypes
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

#[cfg(test)]
#[path = "common_tests.rs"]
mod tests;
