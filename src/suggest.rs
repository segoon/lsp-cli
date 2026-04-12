use std::path::Path;

use crate::config::LspConfig;
use crate::detect::DetectionResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedLanguage {
    pub name: String,
    pub server: String,
    pub command: Vec<String>,
}

pub fn suggestions_for(
    lsps: &[LspConfig],
    detection: &DetectionResult,
    workspace: &Path,
) -> Result<Vec<SuggestedLanguage>, String> {
    lsps.iter()
        .filter(|lsp| matches_lsp(lsp, detection))
        .map(|lsp| build_suggestion(lsp, workspace))
        .collect()
}

fn matches_lsp(lsp: &LspConfig, detection: &DetectionResult) -> bool {
    lsp.filetypes
        .iter()
        .any(|filetype| detection.filetypes.contains(filetype))
        || lsp
            .filepatterns
            .iter()
            .any(|pattern| detection.filenames.contains(pattern))
}

fn build_suggestion(lsp: &LspConfig, workspace: &Path) -> Result<SuggestedLanguage, String> {
    let workspace = workspace.to_string_lossy();
    let template = shlex::split(&lsp.cmdline)
        .ok_or_else(|| format!("invalid cmdline for {}: {}", lsp.name, lsp.cmdline))?;
    let command = template
        .into_iter()
        .map(|token| token.replace("$WORKSPACE", &workspace))
        .collect::<Vec<_>>();

    if command.is_empty() {
        return Err(format!("empty cmdline for {}", lsp.name));
    }

    Ok(SuggestedLanguage {
        name: lsp.name.clone(),
        server: lsp.name.clone(),
        command,
    })
}

#[cfg(test)]
mod tests {
    use super::{SuggestedLanguage, suggestions_for};
    use crate::config::LspConfig;
    use crate::detect::DetectionResult;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn clangd() -> LspConfig {
        LspConfig {
            filetypes: vec!["c".to_string(), "cpp".to_string()],
            filepatterns: vec!["compile_commands.json".to_string()],
            name: "clangd".to_string(),
            cmdline: "clangd --background-index $WORKSPACE".to_string(),
        }
    }

    #[test]
    fn suggests_lsp_from_detected_filetype() {
        let suggestions = suggestions_for(
            &[clangd()],
            &DetectionResult {
                filetypes: BTreeSet::from(["cpp".to_string()]),
                filenames: BTreeSet::new(),
            },
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions,
            vec![SuggestedLanguage {
                name: "clangd".to_string(),
                server: "clangd".to_string(),
                command: vec![
                    "clangd".to_string(),
                    "--background-index".to_string(),
                    ".".to_string()
                ],
            }]
        );
    }

    #[test]
    fn suggests_lsp_from_filepattern() {
        let suggestions = suggestions_for(
            &[clangd()],
            &DetectionResult {
                filetypes: BTreeSet::new(),
                filenames: BTreeSet::from(["compile_commands.json".to_string()]),
            },
            Path::new("workspace"),
        )
        .expect("suggestions should succeed");

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].command[2], "workspace");
    }

    #[test]
    fn returns_no_suggestions_when_nothing_matches() {
        let suggestions = suggestions_for(
            &[clangd()],
            &DetectionResult {
                filetypes: BTreeSet::from(["rust".to_string()]),
                filenames: BTreeSet::from(["Cargo.toml".to_string()]),
            },
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert!(suggestions.is_empty());
    }

    #[test]
    fn substitutes_workspace_without_splitting_spaces() {
        let suggestions = suggestions_for(
            &[clangd()],
            &DetectionResult {
                filetypes: BTreeSet::from(["c".to_string()]),
                filenames: BTreeSet::new(),
            },
            Path::new("/tmp/with spaces"),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].command,
            vec![
                "clangd".to_string(),
                "--background-index".to_string(),
                "/tmp/with spaces".to_string()
            ]
        );
    }
}
