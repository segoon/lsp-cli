use crate::cli::DetectArgs;
use crate::commands::common::analyze_path;
use crate::config::ConfigStore;
use crate::mason::resolve_detect_suggestions;
use crate::suggest::SuggestedLanguage;
use serde_json::json;
use std::collections::BTreeSet;

pub(super) fn run(args: &DetectArgs, config: &ConfigStore) -> Result<String, String> {
    // Q: args.server is duplicated multiple times
    // fix here and search for similar duplication in other functions
    let (detection, suggestions) = analyze_path(&args.path, config)?;
    let suggestions = filter_detect_suggestions(
        &suggestions,
        args.server.selected_language(),
        args.server.selected_server(),
    )?;
    let suggestions = resolve_detect_suggestions(&suggestions, args.server.download)?;
    ensure_requested_suggestions_resolved(
        &suggestions,
        args.server.selected_language(),
        args.server.selected_server(),
    )?;

    Ok(if args.json {
        render_json(&suggestions)
    } else if args.quiet {
        render_quiet(&suggestions)
    } else {
        render_text(&detection.filetypes, &suggestions)
    })
}

fn filter_detect_suggestions(
    suggestions: &[SuggestedLanguage],
    selected_language: Option<&str>,
    selected_server: Option<&str>,
) -> Result<Vec<SuggestedLanguage>, String> {
    let mut filtered = suggestions.to_vec();

    if let Some(selected_server) = selected_server {
        filtered.retain(|suggestion| suggestion.server == selected_server);
        if let Some(selected_language) = selected_language {
            filtered.retain(|suggestion| {
                suggestion
                    .languages
                    .iter()
                    .any(|language| language == selected_language)
            });
            if filtered.is_empty() {
                return Err(
                    if suggestions
                        .iter()
                        .any(|suggestion| suggestion.server == selected_server)
                    {
                        format!(
                            "requested LSP server {selected_server:?} is not available for language {selected_language:?}"
                        )
                    } else {
                        requested_server_not_detected_error(suggestions, selected_server)
                    },
                );
            }
        } else if filtered.is_empty() {
            return Err(requested_server_not_detected_error(
                suggestions,
                selected_server,
            ));
        }

        return Ok(filtered);
    }

    if let Some(selected_language) = selected_language {
        filtered.retain(|suggestion| {
            suggestion
                .languages
                .iter()
                .any(|language| language == selected_language)
        });
        if filtered.is_empty() {
            return Err(format!(
                "no LSP server was detected for language {selected_language:?}"
            ));
        }
    }

    Ok(filtered)
}

fn ensure_requested_suggestions_resolved(
    suggestions: &[SuggestedLanguage],
    selected_language: Option<&str>,
    selected_server: Option<&str>,
) -> Result<(), String> {
    if !suggestions.is_empty() {
        return Ok(());
    }

    match (selected_server, selected_language) {
        (Some(selected_server), Some(selected_language)) => Err(format!(
            "requested LSP server {selected_server:?} is not runnable for language {selected_language:?}"
        )),
        (Some(selected_server), None) => Err(format!(
            "requested LSP server {selected_server:?} is not runnable"
        )),
        (None, Some(selected_language)) => Err(format!(
            "no runnable LSP server was found for language {selected_language:?}"
        )),
        (None, None) => Ok(()),
    }
}

fn requested_server_not_detected_error(
    suggestions: &[SuggestedLanguage],
    selected_server: &str,
) -> String {
    let available = suggestions
        .iter()
        .map(|suggestion| suggestion.server.as_str())
        .collect::<Vec<_>>();
    if available.is_empty() {
        format!(
            "requested LSP server {selected_server:?} is not available because no matching servers were detected"
        )
    } else {
        format!(
            "requested LSP server {selected_server:?} is not in the detected server list: {}",
            available.join(", ")
        )
    }
}

pub(super) fn render_quiet(suggestions: &[SuggestedLanguage]) -> String {
    suggestions
        .iter()
        .map(|suggestion| suggestion.command.join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn render_text(
    detected_filetypes: &BTreeSet<String>,
    suggestions: &[SuggestedLanguage],
) -> String {
    if suggestions.is_empty() {
        return if detected_filetypes.is_empty() {
            "No supported languages detected".to_string()
        } else {
            format!(
                "No runnable LSP server found for detected filetypes: {}",
                detected_filetypes
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
    }

    suggestions
        .iter()
        .map(|suggestion| {
            let languages = suggestion.languages.join(", ");
            format!(
                "Language: {}\nSuggested command: {}",
                languages,
                suggestion.command.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(super) fn render_json(suggestions: &[SuggestedLanguage]) -> String {
    json!({
        "servers": suggestions
            .iter()
            .map(|suggestion| {
                json!({
                    "languages": suggestion.languages,
                    "server": suggestion.server,
                    "command": suggestion.command,
                })
            })
            .collect::<Vec<_>>(),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_requested_suggestions_resolved, filter_detect_suggestions, render_json,
        render_quiet, render_text,
    };
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

    fn secondary_suggestion() -> SuggestedLanguage {
        SuggestedLanguage {
            config_id: "secondary_lsp".to_string(),
            languages: vec!["gamma".to_string()],
            server: "secondary-lsp".to_string(),
            command: vec!["secondary-lsp".to_string(), "--stdio".to_string()],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
        }
    }

    #[test]
    fn filters_detect_suggestions_by_language() {
        assert_eq!(
            filter_detect_suggestions(
                &[example_suggestion(), secondary_suggestion()],
                Some("gamma"),
                None,
            )
            .expect("language filter should succeed"),
            vec![secondary_suggestion()]
        );
    }

    #[test]
    fn filters_detect_suggestions_by_server() {
        assert_eq!(
            filter_detect_suggestions(
                &[example_suggestion(), secondary_suggestion()],
                None,
                Some("example-lsp"),
            )
            .expect("server filter should succeed"),
            vec![example_suggestion()]
        );
    }

    #[test]
    fn filters_detect_suggestions_by_language_and_server() {
        assert_eq!(
            filter_detect_suggestions(&[example_suggestion()], Some("beta"), Some("example-lsp"))
                .expect("combined filter should succeed"),
            vec![example_suggestion()]
        );
    }

    #[test]
    fn errors_when_detect_language_has_no_matching_server() {
        assert_eq!(
            filter_detect_suggestions(&[example_suggestion()], Some("gamma"), None),
            Err("no LSP server was detected for language \"gamma\"".to_string())
        );
    }

    #[test]
    fn errors_when_detect_server_is_not_detected() {
        assert_eq!(
            filter_detect_suggestions(&[example_suggestion()], None, Some("missing-lsp")),
            Err(
                "requested LSP server \"missing-lsp\" is not in the detected server list: example-lsp"
                    .to_string()
            )
        );
    }

    #[test]
    fn errors_when_detect_server_is_not_detected_and_nothing_matches() {
        assert_eq!(
            filter_detect_suggestions(&[], None, Some("missing-lsp")),
            Err(
                "requested LSP server \"missing-lsp\" is not available because no matching servers were detected"
                    .to_string()
            )
        );
    }

    #[test]
    fn errors_when_detect_server_is_not_available_for_language() {
        assert_eq!(
            filter_detect_suggestions(&[example_suggestion()], Some("gamma"), Some("example-lsp")),
            Err(
                "requested LSP server \"example-lsp\" is not available for language \"gamma\""
                    .to_string()
            )
        );
    }

    #[test]
    fn errors_when_requested_detect_server_is_not_runnable() {
        assert_eq!(
            ensure_requested_suggestions_resolved(&[], None, Some("example-lsp")),
            Err("requested LSP server \"example-lsp\" is not runnable".to_string())
        );
    }

    #[test]
    fn errors_when_requested_detect_server_is_not_runnable_for_language() {
        assert_eq!(
            ensure_requested_suggestions_resolved(&[], Some("beta"), Some("example-lsp")),
            Err(
                "requested LSP server \"example-lsp\" is not runnable for language \"beta\""
                    .to_string()
            )
        );
    }

    #[test]
    fn errors_when_requested_detect_language_has_no_runnable_server() {
        assert_eq!(
            ensure_requested_suggestions_resolved(&[], Some("beta"), None),
            Err("no runnable LSP server was found for language \"beta\"".to_string())
        );
    }

    #[test]
    fn renders_empty_detect_text_output() {
        assert_eq!(
            render_text(&BTreeSet::new(), &[]),
            "No supported languages detected"
        );
    }

    #[test]
    fn renders_no_runnable_server_output() {
        assert_eq!(
            render_text(&BTreeSet::from(["python".to_string()]), &[]),
            "No runnable LSP server found for detected filetypes: python"
        );
    }

    #[test]
    fn renders_detect_text_output() {
        let detected = BTreeSet::from(["alpha".to_string(), "beta".to_string()]);

        assert_eq!(
            render_text(&detected, &[example_suggestion()]),
            "Language: alpha, beta\nSuggested command: example-lsp --stdio"
        );
    }

    #[test]
    fn renders_server_specific_languages_in_text_output() {
        let detected =
            BTreeSet::from(["alpha".to_string(), "beta".to_string(), "gamma".to_string()]);
        let suggestion = SuggestedLanguage {
            languages: vec!["beta".to_string()],
            ..example_suggestion()
        };

        assert_eq!(
            render_text(&detected, &[suggestion]),
            "Language: beta\nSuggested command: example-lsp --stdio"
        );
    }

    #[test]
    fn renders_detect_quiet_output() {
        assert_eq!(render_quiet(&[example_suggestion()]), "example-lsp --stdio");
    }

    #[test]
    fn renders_detect_json_output() {
        assert_eq!(
            render_json(&[example_suggestion()]),
            concat!(
                "{\"servers\":[",
                "{\"command\":[\"example-lsp\",\"--stdio\"],\"languages\":[\"alpha\",\"beta\"],\"server\":\"example-lsp\"}",
                "]}"
            )
        );
    }
}
