use std::path::{Path, PathBuf};

use crate::config::LspConfig;
use crate::detect::DetectionResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedLanguage {
    pub languages: Vec<String>,
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
        .map(|lsp| build_suggestion(lsp, detection, workspace))
        .collect()
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
    let workspace = resolve_workspace_root(lsp, workspace)
        .map_err(|error| format!("failed to resolve workspace for {}: {error}", lsp.name))?;
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
        languages: matching_languages(lsp, detection),
        server: lsp.name.clone(),
        command,
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
            .map(Path::to_path_buf)
            .unwrap_or_else(|| workspace.to_path_buf()),
        Ok(_) => workspace.to_path_buf(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => workspace.to_path_buf(),
        Err(error) => return Err(error),
    };

    for directory in start.ancestors() {
        if has_any_root_marker(directory, &lsp.root_markers)? {
            return Ok(directory.to_path_buf());
        }
    }

    Ok(start)
}

fn has_any_root_marker(directory: &Path, root_markers: &[String]) -> std::io::Result<bool> {
    if root_markers.is_empty() {
        return Ok(false);
    }

    if !directory.is_dir() {
        return Ok(false);
    }

    Ok(root_markers
        .iter()
        .any(|marker| directory.join(marker).exists()))
}

#[cfg(test)]
mod tests {
    use super::{SuggestedLanguage, suggestions_for};
    use crate::config::LspConfig;
    use crate::detect::DetectionResult;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn clangd() -> LspConfig {
        LspConfig {
            filetypes: vec!["c".to_string(), "cpp".to_string()],
            root_markers: vec!["compile_commands.json".to_string(), ".git".to_string()],
            name: "clangd".to_string(),
            cmdline: "clangd --background-index $WORKSPACE".to_string(),
        }
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lsp-cli-suggest-test-{}-{}",
                std::process::id(),
                unique
            ));
            std::fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_file(&self, relative: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent dirs should be created");
            }

            std::fs::write(&path, b"test").expect("file should be written");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
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
                languages: vec!["cpp".to_string()],
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
    fn does_not_suggest_lsp_from_root_marker_alone() {
        let suggestions = suggestions_for(
            &[clangd()],
            &DetectionResult {
                filetypes: BTreeSet::new(),
                filenames: BTreeSet::from(["compile_commands.json".to_string()]),
            },
            Path::new("workspace"),
        )
        .expect("suggestions should succeed");

        assert!(suggestions.is_empty());
    }

    #[test]
    fn includes_all_matching_languages() {
        let suggestions = suggestions_for(
            &[clangd()],
            &DetectionResult {
                filetypes: BTreeSet::from(["c".to_string(), "cpp".to_string()]),
                filenames: BTreeSet::new(),
            },
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].languages,
            vec!["c".to_string(), "cpp".to_string()]
        );
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

    #[test]
    fn resolves_workspace_from_root_marker() {
        let dir = TestDir::new();
        dir.write_file("compile_commands.json");
        let nested = dir.write_file("src/main.cpp");

        let suggestions = suggestions_for(
            &[clangd()],
            &DetectionResult {
                filetypes: BTreeSet::from(["cpp".to_string()]),
                filenames: BTreeSet::from([
                    "compile_commands.json".to_string(),
                    "main.cpp".to_string(),
                ]),
            },
            &nested,
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].command,
            vec![
                "clangd".to_string(),
                "--background-index".to_string(),
                dir.path().display().to_string()
            ]
        );
    }

    #[test]
    fn falls_back_to_input_directory_when_no_marker_exists() {
        let dir = TestDir::new();
        let nested = dir.write_file("src/main.cpp");

        let suggestions = suggestions_for(
            &[clangd()],
            &DetectionResult {
                filetypes: BTreeSet::from(["cpp".to_string()]),
                filenames: BTreeSet::from(["main.cpp".to_string()]),
            },
            &nested,
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].command,
            vec![
                "clangd".to_string(),
                "--background-index".to_string(),
                dir.path().join("src").display().to_string()
            ]
        );
    }
}
