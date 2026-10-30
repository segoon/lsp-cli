use super::CACHE_MARKER;
use crate::mason::install::{resolve_cached_program, resolve_or_install_program};
use crate::mason::registry::MasonPackage;
use crate::mason::source::{SourceId, parse_source_id};
use crate::runtime_state::RuntimeState;
use crate::test_support::{TestDir, asm_lsp_package, env_var, make_executable, with_env_vars};
use std::path::Path;

struct Fixture {
    dir: TestDir,
    state: RuntimeState,
    package: MasonPackage,
}

impl Fixture {
    fn new() -> Self {
        let dir = TestDir::new("cargo-cache");
        let state = RuntimeState::new(dir.path().join("state with spaces"));
        Self {
            dir,
            state,
            package: asm_lsp_package(),
        }
    }

    fn marker(&self) -> std::path::PathBuf {
        self.state
            .package_dir(&self.package.name)
            .join(CACHE_MARKER)
    }

    fn executable(&self) -> std::path::PathBuf {
        self.state
            .package_dir(&self.package.name)
            .join("bin/asm-lsp")
    }

    fn install(&self, body: &str) -> crate::error::Result<std::path::PathBuf> {
        self.with_installer(body, || {
            resolve_or_install_program(&self.state, &self.package, "asm-lsp")
        })
    }

    fn with_installer<T>(&self, body: &str, action: impl FnOnce() -> T) -> T {
        let path = self
            .dir
            .write_file("tools/cargo", format!("#!/bin/sh\n{body}\n"));
        make_executable(&path);
        with_env_vars(
            &[env_var(
                "LSP_CLI_INSTALL_PATH",
                path.parent().expect("tool has a parent"),
            )],
            action,
        )
    }

    fn cached(&self) -> Option<std::path::PathBuf> {
        resolve_cached_program(&self.state, &self.package, "asm-lsp")
            .expect("cache lookup should succeed")
    }
}

const INSTALLER: &str = r#"
test "$1" = install || exit 1
test "$2" = --root || exit 1
root=$3
shift 3
printf '%s\n' "$@" >> "$root/args"
/bin/mkdir -p "$root/bin"
printf '#!/bin/sh\nexit 0\n' > "$root/bin/asm-lsp"
/bin/chmod 755 "$root/bin/asm-lsp"
"#;

#[test]
fn builds_registry_tag_revision_and_feature_install_commands() {
    for (id, expected) in [
        (
            "pkg:cargo/asm-lsp@0.10.1",
            vec!["--version", "0.10.1", "--locked", "--force", "asm-lsp"],
        ),
        (
            "pkg:cargo/asm-lsp@v1?repository_url=https://example.com/repo",
            vec![
                "--git",
                "https://example.com/repo",
                "--tag",
                "v1",
                "--locked",
                "--force",
                "asm-lsp",
            ],
        ),
        (
            "pkg:cargo/asm-lsp@abc?repository_url=https://example.com/repo&rev=true&features=lsp,cli&locked=false",
            vec![
                "--git",
                "https://example.com/repo",
                "--rev",
                "abc",
                "--features",
                "lsp,cli",
                "--force",
                "asm-lsp",
            ],
        ),
    ] {
        let SourceId::Cargo(source) = parse_source_id(id).expect("source should parse") else {
            panic!("expected Cargo source")
        };
        let command = source.install_command(Path::new("managed tree"));
        assert_eq!(command.get_program(), "cargo");
        let mut args = vec!["install", "--root", "managed tree"];
        args.extend(expected);
        assert_eq!(command.get_args().collect::<Vec<_>>(), args);
    }
}

#[test]
fn rebuilds_unmarked_or_changed_installations_and_reuses_current_cache() {
    let mut fixture = Fixture::new();
    std::fs::create_dir_all(
        fixture
            .executable()
            .parent()
            .expect("launcher has a parent"),
    )
    .expect("bin directory should exist");
    std::fs::write(fixture.executable(), "old executable").expect("old launcher should exist");
    make_executable(&fixture.executable());
    assert_eq!(fixture.cached(), None);
    assert_eq!(
        fixture
            .install(INSTALLER)
            .expect("old installation should rebuild"),
        fixture.executable()
    );
    assert_eq!(fixture.cached(), Some(fixture.executable()));
    let marker = std::fs::read_to_string(fixture.marker()).expect("signature should exist");
    fixture
        .package
        .source
        .id
        .push_str("?features=lsp,cli&locked=false");
    assert_eq!(fixture.cached(), None);
    fixture
        .install(INSTALLER)
        .expect("changed source should rebuild");
    assert_ne!(
        std::fs::read_to_string(fixture.marker()).expect("new signature should exist"),
        marker
    );
    let args_path = fixture
        .state
        .package_dir(&fixture.package.name)
        .join("args");
    let args = std::fs::read_to_string(&args_path).expect("arguments should exist");
    assert!(args.contains("--features\nlsp,cli\n--force\n"));
    fixture
        .install("exit 99")
        .expect("valid cache must not invoke Cargo");
    assert_eq!(
        std::fs::read_to_string(args_path).expect("arguments should remain"),
        args
    );
}

#[test]
fn failed_rebuilds_cannot_reuse_markers_or_formerly_enabled_binaries() {
    for (script, expected) in [
        ("echo 'build failed' >&2\nexit 1", "build failed"),
        ("exit 0", "cargo did not produce a runnable"),
    ] {
        let mut fixture = Fixture::new();
        fixture
            .install(INSTALLER)
            .expect("initial installation should work");
        let original = fixture.package.source.id.clone();
        fixture.package.source.id.push_str("?features=other");
        let error = fixture
            .install(script)
            .expect_err("rebuild should fail")
            .to_string();
        assert!(error.contains(expected), "{error}");
        assert!(!fixture.marker().exists());
        assert!(!fixture.executable().exists());
        fixture.package.source.id = original;
        assert_eq!(fixture.cached(), None);
        fixture
            .install(INSTALLER)
            .expect("failed build should be recoverable");
    }
}
