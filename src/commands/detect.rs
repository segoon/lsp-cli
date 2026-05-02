use crate::cli::DetectArgs;
use crate::commands::common::analyze_path;
use crate::config::ConfigStore;
use crate::mason::resolve_detect_suggestions;
use crate::suggest::SuggestedLanguage;
use serde_json::json;
use std::collections::BTreeSet;

pub(super) fn run(args: &DetectArgs, config: &ConfigStore) -> Result<String, String> {
    let (detection, suggestions) = analyze_path(&args.path, config)?;
    let suggestions = resolve_detect_suggestions(&suggestions, args.download)?;

    Ok(if args.json {
        render_json(&suggestions)
    } else if args.quiet {
        render_quiet(&suggestions)
    } else {
        render_text(&detection.filetypes, &suggestions)
    })
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
    use super::{render_json, render_quiet, render_text};
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
        let detected = BTreeSet::from([
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
        ]);
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
