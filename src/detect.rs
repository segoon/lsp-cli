use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

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

fn scan_path(
    path: &Path,
    filetypes: &[FiletypeConfig],
    detection: &mut DetectionResult,
) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| path_error(path, error))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        return Ok(());
    }

    if file_type.is_file() {
        detect_file(path, filetypes, detection);
        return Ok(());
    }

    if file_type.is_dir() {
        let entries = fs::read_dir(path).map_err(|error| path_error(path, error))?;

        for entry in entries {
            let entry = entry.map_err(|error| path_error(path, error))?;
            scan_path(&entry.path(), filetypes, detection)?;
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
        .map(|value| value.to_ascii_lowercase());

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

fn path_error(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{DetectionResult, detect_workspace};
    use crate::config::FiletypeConfig;
    use regex::Regex;
    use std::collections::BTreeSet;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lsp-cli-test-{}-{}",
                std::process::id(),
                unique
            ));

            fs::create_dir_all(&path).expect("temp dir should be created");

            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_file(&self, relative: &str) {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dirs should be created");
            }

            fs::write(path, b"test").expect("file should be written");
        }

        #[cfg(unix)]
        fn symlink(&self, target: &str, link: &str) {
            let link_path = self.path.join(link);
            if let Some(parent) = link_path.parent() {
                fs::create_dir_all(parent).expect("parent dirs should be created");
            }

            symlink(target, link_path).expect("symlink should be created");
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn filetype(id: &str, extensions: &[&str], patterns: &[&str]) -> FiletypeConfig {
        FiletypeConfig {
            id: id.to_string(),
            extensions: extensions.iter().map(|value| value.to_string()).collect(),
            patterns: patterns
                .iter()
                .map(|value| Regex::new(value).expect("pattern should compile"))
                .collect(),
        }
    }

    #[test]
    fn detects_filetypes_by_extension() {
        let dir = TestDir::new();
        dir.write_file("src/main.cpp");

        let detection = detect_workspace(dir.path(), &[filetype("cpp", &["cpp"], &[])])
            .expect("scan should succeed");

        assert_eq!(
            detection,
            DetectionResult {
                filetypes: BTreeSet::from(["cpp".to_string()]),
                filenames: BTreeSet::from(["main.cpp".to_string()]),
            }
        );
    }

    #[test]
    fn detects_filetypes_by_pattern() {
        let dir = TestDir::new();
        dir.write_file("src/SConstruct");

        let detection = detect_workspace(dir.path(), &[filetype("cpp", &[], &[r"^SConstruct$"])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["cpp".to_string()]));
    }

    #[test]
    fn detects_objective_c_files() {
        let dir = TestDir::new();
        dir.write_file("src/main.m");

        let detection = detect_workspace(dir.path(), &[filetype("objc", &["m"], &[])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["objc".to_string()]));
    }

    #[test]
    fn detects_objective_cpp_files() {
        let dir = TestDir::new();
        dir.write_file("src/main.mm");

        let detection = detect_workspace(dir.path(), &[filetype("objcpp", &["mm"], &[])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["objcpp".to_string()]));
    }

    #[test]
    fn detects_cuda_files() {
        let dir = TestDir::new();
        dir.write_file("src/kernel.cu");
        dir.write_file("include/kernel.cuh");

        let detection = detect_workspace(dir.path(), &[filetype("cuda", &["cu", "cuh"], &[])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["cuda".to_string()]));
    }

    #[test]
    fn scans_nested_directories() {
        let dir = TestDir::new();
        dir.write_file("deeply/nested/project/source.cxx");

        let detection = detect_workspace(dir.path(), &[filetype("cpp", &["cxx"], &[])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["cpp".to_string()]));
    }

    #[test]
    fn returns_empty_when_no_supported_files_exist() {
        let dir = TestDir::new();
        dir.write_file("README.md");

        let detection = detect_workspace(dir.path(), &[filetype("cpp", &["cpp"], &[])])
            .expect("scan should succeed");

        assert!(detection.filetypes.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn skips_broken_symlinks() {
        let dir = TestDir::new();
        dir.symlink("missing-target", "broken-link");
        dir.write_file("src/main.cpp");

        let detection = detect_workspace(dir.path(), &[filetype("cpp", &["cpp"], &[])])
            .expect("scan should succeed");

        assert_eq!(detection.filetypes, BTreeSet::from(["cpp".to_string()]));
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinked_directories() {
        let dir = TestDir::new();
        dir.write_file("real-src/main.cpp");
        dir.symlink("real-src", "linked-src");

        let detection = detect_workspace(
            &dir.path().join("linked-src"),
            &[filetype("cpp", &["cpp"], &[])],
        )
        .expect("scan should succeed");

        assert!(detection.filetypes.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinked_files() {
        let dir = TestDir::new();
        dir.write_file("real.cpp");
        dir.symlink("real.cpp", "linked.cpp");

        let detection = detect_workspace(
            &dir.path().join("linked.cpp"),
            &[filetype("cpp", &["cpp"], &[])],
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
}
