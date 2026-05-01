use crate::config::ConfigStore;
use crate::detect::{DetectionResult, detect_workspace};
use crate::lsp::path_to_file_uri;
use crate::suggest::{SuggestedLanguage, suggestions_for};
use std::path::Path;

pub(super) struct PreparedWorkspace {
    pub detection: DetectionResult,
    pub server: SuggestedLanguage,
    pub root_uri: String,
    pub workspace_name: String,
}

pub(super) fn analyze_path(
    path: &Path,
    config: &ConfigStore,
) -> Result<(DetectionResult, Vec<SuggestedLanguage>), String> {
    let detection = detect_workspace(path, &config.filetypes)
        .map_err(|error| format!("failed to scan {}: {error}", path.display()))?;
    let suggestions = suggestions_for(&config.lsps, &detection, path)
        .map_err(|error| format!("failed to build suggestions: {error}"))?;

    Ok((detection, suggestions))
}

pub(super) fn prepare_workspace(
    path: &Path,
    selected_server: Option<&str>,
    config: &ConfigStore,
) -> Result<PreparedWorkspace, String> {
    let (detection, suggestions) = analyze_path(path, config)?;
    let server = select_server(&detection, &suggestions, selected_server)?.clone();
    let root_uri = path_to_file_uri(&server.workspace_root)?;
    let workspace_name = crate::lsp::workspace_name(&server.workspace_root);

    Ok(PreparedWorkspace {
        detection,
        server,
        root_uri,
        workspace_name,
    })
}

pub(super) fn select_server<'a>(
    detection: &DetectionResult,
    suggestions: &'a [SuggestedLanguage],
    selected_server: Option<&str>,
) -> Result<&'a SuggestedLanguage, String> {
    if let Some(server) = selected_server {
        return suggestions.iter().find(|suggestion| suggestion.server == server).ok_or_else(|| {
            let available = suggestions
                .iter()
                .map(|suggestion| suggestion.server.as_str())
                .collect::<Vec<_>>();
            if available.is_empty() {
                format!("Requested LSP server {server:?} is not available because no matching servers were detected")
            } else {
                format!(
                    "Requested LSP server {server:?} is not in the detected server list: {}",
                    available.join(", ")
                )
            }
        });
    }

    suggestions.first().ok_or_else(|| {
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
    })
}

#[cfg(test)]
mod tests {
    use super::select_server;
    use crate::detect::DetectionResult;
    use crate::suggest::SuggestedLanguage;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn example_suggestion() -> SuggestedLanguage {
        SuggestedLanguage {
            config_id: "example_lsp".to_string(),
            languages: vec!["alpha".to_string(), "beta".to_string()],
            server: "example-lsp".to_string(),
            command: vec!["example-lsp".to_string(), "--stdio".to_string()],
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
            command: vec!["secondary-lsp".to_string()],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
        };
        let suggestions = [primary, secondary.clone()];

        let selected = select_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            &suggestions,
            Some("secondary-lsp"),
        )
        .expect("requested server should be selected");

        assert_eq!(selected.server, secondary.server);
    }

    #[test]
    fn errors_when_requested_server_is_not_detected() {
        let error = select_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            &[example_suggestion()],
            Some("missing-lsp"),
        )
        .expect_err("missing server should error");

        assert_eq!(
            error,
            "Requested LSP server \"missing-lsp\" is not in the detected server list: example-lsp"
        );
    }
}
