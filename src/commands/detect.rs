use crate::cli::DetectArgs;
use crate::commands::common::analyze_path;
use crate::config::ConfigStore;
use crate::suggest::SuggestedLanguage;
use serde_json::json;
use std::collections::BTreeSet;

pub(super) fn run(args: &DetectArgs, config: &ConfigStore) -> Result<String, String> {
    let (detection, suggestions) = analyze_path(&args.path, config)?;

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
        return "No supported languages detected".to_string();
    }

    let detected = if detected_filetypes.is_empty() {
        "none".to_string()
    } else {
        detected_filetypes
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };

    suggestions
        .iter()
        .map(|suggestion| {
            format!(
                "Detected: {}\nSuggested command: {}",
                detected,
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
    fn renders_detect_text_output() {
        let detected = BTreeSet::from(["alpha".to_string(), "beta".to_string()]);

        assert_eq!(
            render_text(&detected, &[example_suggestion()]),
            "Detected: alpha, beta\nSuggested command: example-lsp --stdio"
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
