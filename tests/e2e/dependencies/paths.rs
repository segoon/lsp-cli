use std::fs;
use std::path::{Path, PathBuf};

pub(super) enum PathEntry {
    Directory(&'static str),
    Alias {
        name: &'static str,
        program: &'static str,
    },
}

impl PathEntry {
    pub(super) fn directory(&self, root: &Path) -> PathBuf {
        match self {
            Self::Directory(directory) => root.join(directory),
            Self::Alias { name, .. } => root.join(".lsp-cli-aliases").join(name),
        }
    }

    pub(super) fn prepare(&self, root: &Path) -> Result<(), String> {
        let Self::Alias { name, program } = self else {
            return Ok(());
        };
        let target = root.join(program);
        if !target.is_file() {
            return Err(format!(
                "cannot expose E2E runtime {name:?}: {} is not a file",
                target.display()
            ));
        }
        // Expose only the named executable, so an alternate runtime cannot shadow default tools.
        let directory = self.directory(root);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("failed to create E2E runtime alias directory: {error}"))?;
        let alias = directory.join(name);
        // Cache restores and linked caches can move the installation to a different checkout.
        let link_target = Path::new("../..").join(program);
        match fs::read_link(&alias) {
            Ok(existing) if existing == link_target => return Ok(()),
            Ok(_) => {
                return Err(format!(
                    "E2E runtime alias {} points to a different executable",
                    alias.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to inspect E2E runtime alias {}: {error}",
                    alias.display()
                ));
            }
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&link_target, &alias)
                .map_err(|error| format!("failed to expose E2E runtime {name:?}: {error}"))
        }
        #[cfg(not(unix))]
        Err("E2E runtime aliases require Unix symbolic links".to_string())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::Command;

    struct RuntimeFixture {
        directory: tempfile::TempDir,
        default: PathBuf,
        alternate: PathBuf,
        alias: PathEntry,
    }

    impl RuntimeFixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("runtime fixture should be created");
            let default = directory.path().join("default");
            let alternate = directory.path().join("alternate");
            for (root, version) in [(&default, "21"), (&alternate, "25")] {
                fs::create_dir_all(root.join("bin")).expect("runtime bin should be created");
                let program = root.join("bin/java");
                fs::write(&program, format!("#!/bin/sh\necho {version}\n"))
                    .expect("runtime executable should be written");
                fs::set_permissions(&program, fs::Permissions::from_mode(0o755))
                    .expect("runtime executable should be runnable");
            }
            Self {
                directory,
                default,
                alternate,
                alias: PathEntry::Alias {
                    name: "java25",
                    program: "bin/java",
                },
            }
        }

        fn prepare(&self) -> PathBuf {
            self.alias
                .prepare(&self.alternate)
                .expect("runtime alias should be prepared");
            self.alias.directory(&self.alternate).join("java25")
        }

        fn run(&self, program: &str) -> String {
            let path = std::env::join_paths([
                self.alias.directory(&self.alternate),
                self.default.join("bin"),
            ])
            .expect("runtime PATH should join");
            let output = Command::new(program)
                .env("PATH", path)
                .output()
                .expect("runtime should start");
            assert!(output.status.success());
            String::from_utf8(output.stdout).expect("version should be UTF-8")
        }
    }

    #[test]
    fn alternate_runtime_does_not_shadow_default_even_when_first_on_path() {
        let fixture = RuntimeFixture::new();
        fixture.prepare();
        assert_eq!(fixture.run("java"), "21\n");
        assert_eq!(fixture.run("java25"), "25\n");
    }

    #[test]
    fn cached_alias_is_reusable_and_missing_alias_is_recreated() {
        let fixture = RuntimeFixture::new();
        let alias = fixture.prepare();
        fixture.prepare();
        fs::remove_file(&alias).expect("cached alias should be removable");
        fixture.prepare();
        assert_eq!(
            fs::read_link(alias).expect("alias should exist"),
            PathBuf::from("../../bin/java")
        );
    }

    #[test]
    fn alias_survives_restoring_the_runtime_into_another_directory() {
        let mut fixture = RuntimeFixture::new();
        fixture.prepare();
        let restored = fixture.directory.path().join("restored");
        fs::rename(&fixture.alternate, &restored).expect("runtime cache should move");
        fixture.alternate = restored;
        fixture.prepare();
        assert_eq!(fixture.run("java25"), "25\n");
        assert_eq!(fixture.run("java"), "21\n");
    }

    #[test]
    fn conflicting_alias_is_reported_without_overwriting_it() {
        let fixture = RuntimeFixture::new();
        let alias = fixture.prepare();
        fs::remove_file(&alias).expect("cached alias should be removable");
        std::os::unix::fs::symlink(fixture.default.join("bin/java"), &alias)
            .expect("conflicting alias should be created");
        let error = fixture
            .alias
            .prepare(&fixture.alternate)
            .expect_err("conflict should fail");
        assert!(error.contains("points to a different executable"));
        assert_eq!(fixture.run("java25"), "21\n");
    }
}
