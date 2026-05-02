use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::LspConfig;
use crate::detect::DetectionResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedLanguage {
    pub config_id: String,
    pub languages: Vec<String>,
    pub server: String,
    pub command: Vec<String>,
    pub workspace_root: PathBuf,
    pub wait_for_index: bool,
}

pub fn suggestions_for(
    lsps: &[LspConfig],
    detection: &DetectionResult,
    workspace: &Path,
) -> Result<Vec<SuggestedLanguage>, String> {
    lsps.iter()
        .filter(|lsp| matches_lsp(lsp, detection))
        .map(|lsp| build_suggestion(lsp, detection, workspace))
        .collect()
}

pub fn sort_suggestions(
    suggestions: &mut [SuggestedLanguage],
    preferences: &BTreeMap<String, Vec<String>>,
    language: Option<&str>,
) {
    suggestions.sort_by_key(|suggestion| preference_rank(suggestion, preferences, language));
}

fn preference_rank(
    suggestion: &SuggestedLanguage,
    preferences: &BTreeMap<String, Vec<String>>,
    language: Option<&str>,
) -> (usize, usize) {
    let matched_rank = match language {
        Some(language) => preference_index(language, &suggestion.server, preferences),
        None => suggestion
            .languages
            .iter()
            .filter_map(|language| preference_index(language, &suggestion.server, preferences))
            .min(),
    };

    matched_rank.map_or((1, usize::MAX), |rank| (0, rank))
}

fn preference_index(
    language: &str,
    server: &str,
    preferences: &BTreeMap<String, Vec<String>>,
) -> Option<usize> {
    preferences
        .get(language)
        .and_then(|servers| servers.iter().position(|candidate| candidate == server))
}

fn matches_lsp(lsp: &LspConfig, detection: &DetectionResult) -> bool {
    lsp.filetypes
        .iter()
        .any(|filetype| detection.filetypes.contains(filetype))
}

fn build_suggestion(
    lsp: &LspConfig,
    detection: &DetectionResult,
    workspace: &Path,
) -> Result<SuggestedLanguage, String> {
    let workspace_root = resolve_workspace_root(lsp, workspace)
        .map_err(|error| format!("failed to resolve workspace for {}: {error}", lsp.name))?;
    let workspace_root = absolute_path(&workspace_root)
        .map_err(|error| format!("failed to resolve workspace for {}: {error}", lsp.name))?;
    let workspace = workspace_root.to_string_lossy();
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
        config_id: lsp.id.clone(),
        languages: matching_languages(lsp, detection),
        server: lsp.name.clone(),
        command,
        workspace_root,
        wait_for_index: lsp.wait_for_index,
    })
}

fn matching_languages(lsp: &LspConfig, detection: &DetectionResult) -> Vec<String> {
    lsp.filetypes
        .iter()
        .filter(|filetype| detection.filetypes.contains(*filetype))
        .cloned()
        .collect()
}

fn resolve_workspace_root(lsp: &LspConfig, workspace: &Path) -> std::io::Result<PathBuf> {
    let start = match std::fs::metadata(workspace) {
        Ok(metadata) if metadata.is_file() => workspace
            .parent()
            .map_or_else(|| workspace.to_path_buf(), Path::to_path_buf),
        Ok(_) => workspace.to_path_buf(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => workspace.to_path_buf(),
        Err(error) => return Err(error),
    };

    for directory in start.ancestors() {
        if has_any_root_marker(directory, &lsp.root_markers) {
            return Ok(directory.to_path_buf());
        }
    }

    Ok(start)
}

fn absolute_path(path: &Path) -> std::io::Result<PathBuf> {
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && path.is_relative() => {
            Ok(std::env::current_dir()?.join(path))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(error),
    }
}

fn has_any_root_marker(directory: &Path, root_markers: &[String]) -> bool {
    if root_markers.is_empty() {
        return false;
    }

    if !directory.is_dir() {
        return false;
    }

    root_markers
        .iter()
        .any(|marker| directory.join(marker).exists())
}

#[cfg(test)]
mod tests {
    use super::{SuggestedLanguage, sort_suggestions, suggestions_for};
    use crate::config::LspConfig;
    use crate::test_support::{TestDir, detection_result};
    use std::collections::BTreeMap;
    use std::path::Path;

    fn example_lsp() -> LspConfig {
        LspConfig {
            id: "example_lsp".to_string(),
            filetypes: vec!["alpha".to_string(), "beta".to_string()],
            root_markers: vec![".workspace-root".to_string(), ".git".to_string()],
            name: "example-lsp".to_string(),
            cmdline: "example-lsp --stdio $WORKSPACE".to_string(),
            wait_for_index: false,
        }
    }

    fn suggestion(server: &str) -> SuggestedLanguage {
        SuggestedLanguage {
            config_id: server.to_string(),
            languages: vec!["python".to_string()],
            server: server.to_string(),
            command: vec![server.to_string()],
            workspace_root: Path::new(".").to_path_buf(),
            wait_for_index: false,
        }
    }

    #[test]
    fn suggests_lsp_from_detected_filetype() {
        let current_dir = std::env::current_dir().expect("current dir should resolve");
        let suggestions = suggestions_for(
            &[example_lsp()],
            &detection_result(&["beta"], &[]),
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions,
            vec![SuggestedLanguage {
                config_id: "example_lsp".to_string(),
                languages: vec!["beta".to_string()],
                server: "example-lsp".to_string(),
                command: vec![
                    "example-lsp".to_string(),
                    "--stdio".to_string(),
                    current_dir.display().to_string()
                ],
                workspace_root: current_dir,
                wait_for_index: false,
            }]
        );
    }

    #[test]
    fn carries_wait_for_index_from_config() {
        let mut lsp = example_lsp();
        lsp.wait_for_index = true;

        let suggestions =
            suggestions_for(&[lsp], &detection_result(&["beta"], &[]), Path::new("."))
                .expect("suggestions should succeed");

        assert!(suggestions[0].wait_for_index);
    }

    #[test]
    fn does_not_suggest_lsp_from_root_marker_alone() {
        let suggestions = suggestions_for(
            &[example_lsp()],
            &detection_result(&[], &[".workspace-root"]),
            Path::new("workspace"),
        )
        .expect("suggestions should succeed");

        assert!(suggestions.is_empty());
    }

    #[test]
    fn includes_all_matching_languages() {
        let suggestions = suggestions_for(
            &[example_lsp()],
            &detection_result(&["alpha", "beta"], &[]),
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].languages,
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[test]
    fn returns_no_suggestions_when_nothing_matches() {
        let suggestions = suggestions_for(
            &[example_lsp()],
            &detection_result(&["gamma"], &["workspace.lock"]),
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert!(suggestions.is_empty());
    }

    #[test]
    fn substitutes_workspace_without_splitting_spaces() {
        let workspace = std::env::current_dir()
            .expect("current dir should resolve")
            .join("/tmp/with spaces");
        let suggestions = suggestions_for(
            &[example_lsp()],
            &detection_result(&["alpha"], &[]),
            Path::new("/tmp/with spaces"),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].command,
            vec![
                "example-lsp".to_string(),
                "--stdio".to_string(),
                workspace.display().to_string()
            ]
        );
    }

    #[test]
    fn substitutes_relative_workspace_as_absolute_path() {
        let current_dir = std::env::current_dir().expect("current dir should resolve");
        let suggestions = suggestions_for(
            &[example_lsp()],
            &detection_result(&["alpha"], &[]),
            Path::new("playground/c"),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].command,
            vec![
                "example-lsp".to_string(),
                "--stdio".to_string(),
                current_dir.join("playground/c").display().to_string()
            ]
        );
        assert_eq!(
            suggestions[0].workspace_root,
            current_dir.join("playground/c")
        );
    }

    #[test]
    fn resolves_workspace_from_root_marker() {
        let dir = TestDir::new("suggest");
        dir.write_file(".workspace-root", "test");
        let nested = dir.write_file("src/main.foo", "test");

        let suggestions = suggestions_for(
            &[example_lsp()],
            &detection_result(&["beta"], &[".workspace-root", "main.foo"]),
            &nested,
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].command,
            vec![
                "example-lsp".to_string(),
                "--stdio".to_string(),
                dir.path().display().to_string()
            ]
        );
        assert_eq!(suggestions[0].workspace_root, dir.path().to_path_buf());
    }

    #[test]
    fn falls_back_to_input_directory_when_no_marker_exists() {
        let dir = TestDir::new("suggest");
        let nested = dir.write_file("src/main.foo", "test");

        let suggestions = suggestions_for(
            &[example_lsp()],
            &detection_result(&["beta"], &["main.foo"]),
            &nested,
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].command,
            vec![
                "example-lsp".to_string(),
                "--stdio".to_string(),
                dir.path().join("src").display().to_string()
            ]
        );
        assert_eq!(suggestions[0].workspace_root, dir.path().join("src"));
    }

    #[test]
    fn sorts_suggestions_by_language_preference() {
        let mut suggestions = vec![suggestion("pyright"), suggestion("ty")];

        sort_suggestions(
            &mut suggestions,
            &BTreeMap::from([(
                "python".to_string(),
                vec!["ty".to_string(), "pyright".to_string()],
            )]),
            Some("python"),
        );

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.server.as_str())
                .collect::<Vec<_>>(),
            vec!["ty", "pyright"]
        );
    }

    #[test]
    fn keeps_unlisted_servers_after_listed_ones() {
        let mut suggestions = vec![suggestion("jedi-language-server"), suggestion("pyright")];

        sort_suggestions(
            &mut suggestions,
            &BTreeMap::from([("python".to_string(), vec!["pyright".to_string()])]),
            Some("python"),
        );

        assert_eq!(
            suggestions
                .iter()
                .map(|suggestion| suggestion.server.as_str())
                .collect::<Vec<_>>(),
            vec!["pyright", "jedi-language-server"]
        );
    }
}
