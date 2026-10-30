use crate::mason::install::{resolve_cached_program, resolve_or_install_program};
use crate::mason::link::{ResolvedProgram, resolve_program};
use crate::mason::registry::MasonPackage;
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use crate::test_support::{TestDir, cue_package};

struct Fixture {
    dir: TestDir,
    state: RuntimeState,
    package: MasonPackage,
}

impl Fixture {
    fn new() -> Self {
        let dir = TestDir::new("mason-luarocks");
        let state = RuntimeState::new(dir.path().join("state with spaces"));
        let mut package = cue_package("pkg:luarocks/fennel-ls@0.2.1-1");
        package.name = "fennel-ls".to_string();
        package.bin = [("fennel-ls".to_string(), "luarocks:fennel-ls".to_string())].into();
        Self {
            dir,
            state,
            package,
        }
    }

    fn resolve(&self) -> crate::error::Result<ResolvedProgram> {
        resolve_program(
            &self.package,
            "fennel-ls",
            &self.state,
            &TemplateContext::empty(),
        )
    }

    fn executable(&self) -> std::path::PathBuf {
        let name = if cfg!(windows) {
            "fennel-ls.bat"
        } else {
            "fennel-ls"
        };
        self.state.package_dir("fennel-ls").join("bin").join(name)
    }

    #[cfg(unix)]
    fn install_with(&self, script: Option<&str>) -> crate::error::Result<std::path::PathBuf> {
        use crate::test_support::{env_var, make_executable, with_env_vars};
        let tools = self.dir.path().join("tools");
        crate::fs::create_dir_all(&tools)?;
        if let Some(body) = script {
            let installer = self
                .dir
                .write_file("tools/luarocks", format!("#!/bin/sh\n{body}\n"));
            make_executable(&installer);
        }
        // Keep the ordinary PATH empty: the existing installer-path override must be used.
        with_env_vars(
            &[env_var("PATH", ""), env_var("LSP_CLI_INSTALL_PATH", &tools)],
            || resolve_or_install_program(&self.state, &self.package, "fennel-ls"),
        )
    }
}

#[test]
fn resolves_luarocks_launchers_and_rejects_escaping_paths() {
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture
            .resolve()
            .expect("launcher should resolve")
            .executable_path(),
        fixture.executable()
    );
    for target in ["luarocks:../outside", "luarocks:/outside"] {
        fixture
            .package
            .bin
            .insert("fennel-ls".to_string(), target.to_string());
        assert!(
            fixture.resolve().is_err(),
            "{target} should not escape the bin directory"
        );
    }
}

#[cfg(unix)]
#[test]
fn installs_with_qualifiers_and_reuses_cached_launcher() {
    for (source, expected) in [
        ("pkg:luarocks/fennel-ls@0.2.1-1", "fennel-ls\n0.2.1-1\n"),
        (
            "pkg:luarocks/user/fennel-ls@dev-1?dev=true&repository_url=https%3A%2F%2Fluarocks.org%2Fdev",
            "--dev\n--server=https://luarocks.org/dev\nuser/fennel-ls\ndev-1\n",
        ),
    ] {
        let mut fixture = Fixture::new();
        fixture.package.source.id = source.to_string();
        assert_eq!(
            resolve_cached_program(&fixture.state, &fixture.package, "fennel-ls")
                .expect("cache lookup should succeed"),
            None
        );
        let installed = fixture
            .install_with(Some(
                r#"
test "$1" = install || exit 1
test "$2" = --tree || exit 1
tree=$3
shift 3
/bin/mkdir -p "$tree/bin"
printf '%s\n' "$@" > "$tree/args"
printf '#!/bin/sh\nexit 0\n' > "$tree/bin/fennel-ls"
/bin/chmod 755 "$tree/bin/fennel-ls"
"#,
            ))
            .expect("LuaRocks package should install");
        assert_eq!(installed, fixture.executable());
        assert_eq!(
            std::fs::read_to_string(fixture.state.package_dir("fennel-ls").join("args"))
                .expect("installer arguments should be recorded"),
            expected
        );
        let receipt = std::fs::read_to_string(fixture.state.receipt_path("fennel-ls"))
            .expect("receipt should exist");
        assert!(receipt.contains(source));
        assert_eq!(
            resolve_cached_program(&fixture.state, &fixture.package, "fennel-ls")
                .expect("cache lookup should succeed"),
            Some(installed.clone())
        );
        std::fs::remove_file(fixture.dir.path().join("tools/luarocks"))
            .expect("installer should be removed");
        assert_eq!(
            fixture
                .install_with(None)
                .expect("cached server should work without LuaRocks"),
            installed
        );
        assert!(
            std::process::Command::new(installed)
                .status()
                .expect("installed launcher should run")
                .success()
        );
    }
}

#[cfg(unix)]
#[test]
fn reports_missing_installer_failed_install_and_missing_launcher() {
    for (script, expected) in [
        (None, "luarocks is not available in $PATH"),
        (
            Some("echo 'Lua version is incompatible' >&2\nexit 1"),
            "Lua version is incompatible",
        ),
        (
            Some("exit 0"),
            "LuaRocks did not produce a runnable fennel-ls executable",
        ),
    ] {
        let fixture = Fixture::new();
        let error = fixture
            .install_with(script)
            .expect_err("installation should fail")
            .to_string();
        assert!(error.contains(expected), "{error}");
        assert!(!fixture.state.receipt_path("fennel-ls").exists());
    }
}
