use std::path::{Path, PathBuf};

use crate::config::LspConfig;
use crate::detect::DetectionResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestedLanguage {
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
    use super::{SuggestedLanguage, suggestions_for};
    use crate::config::LspConfig;
    use crate::detect::DetectionResult;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn example_lsp() -> LspConfig {
        LspConfig {
            filetypes: vec!["alpha".to_string(), "beta".to_string()],
            root_markers: vec![".workspace-root".to_string(), ".git".to_string()],
            name: "example-lsp".to_string(),
            cmdline: "example-lsp --stdio $WORKSPACE".to_string(),
            wait_for_index: false,
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
            &[example_lsp()],
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions,
            vec![SuggestedLanguage {
                languages: vec!["beta".to_string()],
                server: "example-lsp".to_string(),
                command: vec![
                    "example-lsp".to_string(),
                    "--stdio".to_string(),
                    ".".to_string()
                ],
                workspace_root: PathBuf::from("."),
                wait_for_index: false,
            }]
        );
    }

    #[test]
    fn carries_wait_for_index_from_config() {
        let mut lsp = example_lsp();
        lsp.wait_for_index = true;

        let suggestions = suggestions_for(
            &[lsp],
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert!(suggestions[0].wait_for_index);
    }

    #[test]
    fn does_not_suggest_lsp_from_root_marker_alone() {
        let suggestions = suggestions_for(
            &[example_lsp()],
            &DetectionResult {
                filetypes: BTreeSet::new(),
                filenames: BTreeSet::from([".workspace-root".to_string()]),
            },
            Path::new("workspace"),
        )
        .expect("suggestions should succeed");

        assert!(suggestions.is_empty());
    }

    #[test]
    fn includes_all_matching_languages() {
        let suggestions = suggestions_for(
            &[example_lsp()],
            &DetectionResult {
                filetypes: BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
                filenames: BTreeSet::new(),
            },
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
            &DetectionResult {
                filetypes: BTreeSet::from(["gamma".to_string()]),
                filenames: BTreeSet::from(["workspace.lock".to_string()]),
            },
            Path::new("."),
        )
        .expect("suggestions should succeed");

        assert!(suggestions.is_empty());
    }

    #[test]
    fn substitutes_workspace_without_splitting_spaces() {
        let suggestions = suggestions_for(
            &[example_lsp()],
            &DetectionResult {
                filetypes: BTreeSet::from(["alpha".to_string()]),
                filenames: BTreeSet::new(),
            },
            Path::new("/tmp/with spaces"),
        )
        .expect("suggestions should succeed");

        assert_eq!(
            suggestions[0].command,
            vec![
                "example-lsp".to_string(),
                "--stdio".to_string(),
                "/tmp/with spaces".to_string()
            ]
        );
    }

    #[test]
    fn resolves_workspace_from_root_marker() {
        let dir = TestDir::new();
        dir.write_file(".workspace-root");
        let nested = dir.write_file("src/main.foo");

        let suggestions = suggestions_for(
            &[example_lsp()],
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::from([".workspace-root".to_string(), "main.foo".to_string()]),
            },
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
        let dir = TestDir::new();
        let nested = dir.write_file("src/main.foo");

        let suggestions = suggestions_for(
            &[example_lsp()],
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::from(["main.foo".to_string()]),
            },
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
}
