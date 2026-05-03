use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::FiletypeConfig;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct DetectionResult {
    pub filetypes: BTreeSet<String>,
    pub filenames: BTreeSet<String>,
}

pub fn detect_workspace(path: &Path, filetypes: &[FiletypeConfig]) -> io::Result<DetectionResult> {
    let mut detection = DetectionResult::default();
    scan_path(path, filetypes, &mut detection)?;
    Ok(detection)
}

pub fn matching_files(
    path: &Path,
    filetypes: &[FiletypeConfig],
    allowed_filetypes: &BTreeSet<String>,
) -> io::Result<Vec<PathBuf>> {
    let mut matches = Vec::new();
    collect_matching_files(path, filetypes, allowed_filetypes, &mut matches)?;
    matches.sort();
    Ok(matches)
}

fn scan_path(
    path: &Path,
    filetypes: &[FiletypeConfig],
    detection: &mut DetectionResult,
) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| path_error(path, &error))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        return Ok(());
    }

    if file_type.is_file() {
        detect_file(path, filetypes, detection);
        return Ok(());
    }

    if file_type.is_dir() {
        let entries = fs::read_dir(path).map_err(|error| path_error(path, &error))?;

        for entry in entries {
            let entry = entry.map_err(|error| path_error(path, &error))?;
            scan_path(&entry.path(), filetypes, detection)?;
        }
    }

    Ok(())
}

fn collect_matching_files(
    path: &Path,
    filetypes: &[FiletypeConfig],
    allowed_filetypes: &BTreeSet<String>,
    matches: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| path_error(path, &error))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        return Ok(());
    }

    if file_type.is_file() {
        if file_matches(path, filetypes, allowed_filetypes) {
            matches.push(path.to_path_buf());
        }
        return Ok(());
    }

    if file_type.is_dir() {
        let entries = fs::read_dir(path).map_err(|error| path_error(path, &error))?;

        for entry in entries {
            let entry = entry.map_err(|error| path_error(path, &error))?;
            collect_matching_files(&entry.path(), filetypes, allowed_filetypes, matches)?;
        }
    }

    Ok(())
}

fn detect_file(path: &Path, filetypes: &[FiletypeConfig], detection: &mut DetectionResult) {
    let file_name = match path.file_name() {
        Some(file_name) => file_name.to_string_lossy().into_owned(),
        None => return,
    };

    detection.filenames.insert(file_name.clone());

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);

    for filetype in filetypes {
        let extension_match = extension
            .as_deref()
            .is_some_and(|value| filetype.extensions.iter().any(|ext| ext == value));
        let pattern_match = filetype
            .patterns
            .iter()
            .any(|pattern| pattern.is_match(&file_name));

        if extension_match || pattern_match {
            detection.filetypes.insert(filetype.id.clone());
        }
    }
}

fn file_matches(
    path: &Path,
    filetypes: &[FiletypeConfig],
    allowed_filetypes: &BTreeSet<String>,
) -> bool {
    matching_filetypes(path, filetypes)
        .into_iter()
        .any(|filetype| allowed_filetypes.contains(&filetype))
}

fn matching_filetypes(path: &Path, filetypes: &[FiletypeConfig]) -> Vec<String> {
    let file_name = match path.file_name() {
        Some(file_name) => file_name.to_string_lossy().into_owned(),
        None => return Vec::new(),
    };

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);

    filetypes
        .iter()
        .filter(|filetype| {
            let extension_match = extension
                .as_deref()
                .is_some_and(|value| filetype.extensions.iter().any(|ext| ext == value));
            let pattern_match = filetype
                .patterns
                .iter()
                .any(|pattern| pattern.is_match(&file_name));

            extension_match || pattern_match
        })
        .map(|filetype| filetype.id.clone())
        .collect()
}

fn path_error(path: &Path, error: &io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{DetectionResult, detect_workspace, matching_files};
    use crate::config::FiletypeConfig;
    use crate::test_support::TestDir;
    use regex::Regex;
    use std::collections::BTreeSet;
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn write_file(dir: &TestDir, relative: &str) {
        dir.write_file(relative, "test");
    }

    #[cfg(unix)]
    fn write_symlink(dir: &TestDir, target: &str, link: &str) {
        let link_path = dir.path().join(link);
        if let Some(parent) = link_path.parent() {
            fs::create_dir_all(parent).expect("parent dirs should be created");
        }

        symlink(target, link_path).expect("symlink should be created");
    }

    fn filetype(id: &str, extensions: &[&str], patterns: &[&str]) -> FiletypeConfig {
        FiletypeConfig {
            id: id.to_string(),
            extensions: extensions
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            patterns: patterns
                .iter()
                .map(|value| Regex::new(value).expect("pattern should compile"))
                .collect(),
        }
    }

    #[test]
    fn detects_filetypes_by_extension() {
        let dir = TestDir::new("detect");
        write_file(&dir, "src/main.foo");

        let detection = detect_workspace(dir.path(), &[filetype("alpha", &["foo"], &[])])
            .expect("scan should succeed");

        assert_eq!(
            detection,
            DetectionResult {
                filetypes: BTreeSet::from(["alpha".to_string()]),
                filenames: BTreeSet::from(["main.foo".to_string()]),
            }
        );
    }

    #[test]
    fn detects_filetypes_by_pattern() {
        let dir = TestDir::new("detect");
        write_file(&dir, "src/tooling.config");

        let detection = detect_workspace(
            dir.path(),
            &[filetype("alpha", &[], &[r"^tooling\.config$"])],
        )
        .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["alpha".to_string()]));
    }

    #[test]
    fn detects_multiple_extensions_for_one_filetype() {
        let dir = TestDir::new("detect");
        write_file(&dir, "src/main.bar");
        write_file(&dir, "include/main.baz");

        let detection = detect_workspace(dir.path(), &[filetype("beta", &["bar", "baz"], &[])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["beta".to_string()]));
    }

    #[test]
    fn scans_nested_directories() {
        let dir = TestDir::new("detect");
        write_file(&dir, "deeply/nested/project/source.foo");

        let detection = detect_workspace(dir.path(), &[filetype("alpha", &["foo"], &[])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["alpha".to_string()]));
    }

    #[test]
    fn returns_empty_when_no_supported_files_exist() {
        let dir = TestDir::new("detect");
        write_file(&dir, "README.md");

        let detection = detect_workspace(dir.path(), &[filetype("alpha", &["foo"], &[])])
            .expect("scan should succeed");

        assert!(detection.filetypes.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn skips_broken_symlinks() {
        let dir = TestDir::new("detect");
        write_symlink(&dir, "missing-target", "broken-link");
        write_file(&dir, "src/main.foo");

        let detection = detect_workspace(dir.path(), &[filetype("alpha", &["foo"], &[])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["alpha".to_string()]));
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinked_directories() {
        let dir = TestDir::new("detect");
        write_file(&dir, "real-src/main.foo");
        write_symlink(&dir, "real-src", "linked-src");

        let detection = detect_workspace(
            &dir.path().join("linked-src"),
            &[filetype("alpha", &["foo"], &[])],
        )
        .expect("scan should succeed");

        assert!(detection.filetypes.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinked_files() {
        let dir = TestDir::new("detect");
        write_file(&dir, "real.foo");
        write_symlink(&dir, "real.foo", "linked.foo");

        let detection = detect_workspace(
            &dir.path().join("linked.foo"),
            &[filetype("alpha", &["foo"], &[])],
        )
        .expect("scan should succeed");

        assert!(detection.filetypes.is_empty());
    }

    #[test]
    fn returns_error_for_missing_root_path() {
        let missing = std::env::temp_dir().join(format!(
            "lsp-cli-missing-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        ));

        let error = detect_workspace(&missing, &[]).expect_err("scan should fail");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains(&missing.display().to_string()));
    }

    #[test]
    fn collects_matching_files_for_allowed_filetypes() {
        let dir = TestDir::new("detect");
        write_file(&dir, "src/main.foo");
        write_file(&dir, "src/lib.bar");
        write_file(&dir, "README.md");

        let matches = matching_files(
            dir.path(),
            &[
                filetype("alpha", &["foo"], &[]),
                filetype("beta", &["bar"], &[]),
            ],
            &BTreeSet::from(["alpha".to_string()]),
        )
        .expect("matching files should succeed");

        assert_eq!(matches, vec![dir.path().join("src/main.foo")]);
    }
}
