use super::{installer_command, prepare_install_dir, require_command, run_install_command};
use crate::error::{Error, Result};
use crate::mason::link::{finalize_install, is_resolved_program_runnable, resolve_program};
use crate::mason::registry::MasonPackage;
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use std::path::{Path, PathBuf};
use std::process::Command;

const VENV_DIRECTORY: &str = "local";
const VENV_LAYOUT_MARKER: &str = ".lsp-cli-pypi-venv-v2";

pub(super) fn resolve_cached_program(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
) -> Result<Option<PathBuf>> {
    let resolved = resolve_program(package, program, state, &TemplateContext::empty())?;
    Ok(
        (has_current_layout(state, package) && is_resolved_program_runnable(&resolved))
            .then(|| resolved.executable_path().to_path_buf()),
    )
}

pub(super) fn install_pypi_package(
    state: &RuntimeState,
    package: &MasonPackage,
    package_name: &str,
    version: &str,
    extras: &[String],
    program: &str,
) -> Result<PathBuf> {
    if let Some(cached) = resolve_cached_program(state, package, program)? {
        return Ok(cached);
    }

    require_command("python3", package, program)?;
    let install_dir = prepare_install_dir(state, package)?;
    let venv_dir = install_dir.join(VENV_DIRECTORY);
    let mut create = venv_create_command(&venv_dir);
    run_install_command(&mut create, package, "python3 -m venv")?;

    let python = venv_python(&venv_dir);
    if !python.is_file() {
        return Err(Error::unexpected(format!(
            "cannot install {} because Python did not create the virtual-environment interpreter {}; ensure the Python venv and ensurepip modules are installed",
            package.name,
            python.display()
        )));
    }
    let mut install = pypi_install_command(
        &python,
        package_name,
        version,
        extras,
        &package.source.extra_packages,
    );
    run_install_command(&mut install, package, "virtual-environment pip")?;

    let resolved = resolve_program(package, program, state, &TemplateContext::empty())?;
    let installed = finalize_install(
        state,
        package,
        program,
        &resolved,
        &TemplateContext::empty(),
        "virtual-environment pip did not produce a runnable",
    )?;
    crate::fs::write(
        &layout_marker(state, package),
        layout_signature(package).as_bytes(),
    )?;
    Ok(installed)
}

fn has_current_layout(state: &RuntimeState, package: &MasonPackage) -> bool {
    std::fs::read_to_string(layout_marker(state, package))
        .is_ok_and(|contents| contents == layout_signature(package))
}

fn layout_signature(package: &MasonPackage) -> String {
    // Debug formatting keeps this private cache marker deterministic and unambiguous without
    // introducing a second persisted-data schema. Any source or dependency change rebuilds only
    // this package's virtual environment.
    format!(
        "{:?}\n{:?}\n",
        package.source.id, package.source.extra_packages
    )
}

fn layout_marker(state: &RuntimeState, package: &MasonPackage) -> PathBuf {
    state
        .package_dir(&package.name)
        .join(VENV_DIRECTORY)
        .join(VENV_LAYOUT_MARKER)
}

fn venv_create_command(venv_dir: &Path) -> Command {
    let mut command = installer_command("python3");
    // --clear invalidates only a stale environment for this package, including the old prefix
    // layout, while preserving all other cached package installations.
    command.arg("-m").arg("venv").arg("--clear").arg(venv_dir);
    command
}

fn pypi_install_command(
    python: &Path,
    package_name: &str,
    version: &str,
    extras: &[String],
    extra_packages: &[String],
) -> Command {
    let install_spec = if extras.is_empty() {
        format!("{package_name}=={version}")
    } else {
        format!("{package_name}[{}]=={version}", extras.join(","))
    };
    let mut command = Command::new(python);
    command
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--disable-pip-version-check")
        .arg(install_spec)
        .args(extra_packages);
    command
}

fn venv_python(venv_dir: &Path) -> PathBuf {
    venv_dir.join("bin/python3")
}

#[cfg(test)]
mod tests {
    use super::{VENV_LAYOUT_MARKER, pypi_install_command, venv_create_command};
    use crate::mason::install::resolve_or_install_program;
    use crate::mason::registry::{MasonNeovim, MasonPackage, MasonSource};
    use crate::runtime_state::RuntimeState;
    use crate::test_support::{TestDir, env_var, make_executable, with_env_vars};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn builds_virtual_environment_and_pip_commands() {
        let create = venv_create_command(Path::new("managed/python/local"));
        assert_eq!(create.get_program(), "python3");
        assert_eq!(
            create.get_args().collect::<Vec<_>>(),
            ["-m", "venv", "--clear", "managed/python/local"]
        );

        let install = pypi_install_command(
            Path::new("managed/python/local/bin/python3"),
            "python-lsp-server",
            "1.15.0",
            &["all".to_string()],
            &["pygls<2".to_string()],
        );
        assert_eq!(install.get_program(), "managed/python/local/bin/python3");
        assert_eq!(
            install.get_args().collect::<Vec<_>>(),
            [
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "python-lsp-server[all]==1.15.0",
                "pygls<2",
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn replaces_a_prefix_cache_and_runs_the_virtual_environment_launcher() {
        let dir = TestDir::new("mason-pypi-venv");
        let state = RuntimeState::new(dir.path().join("state"));
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).expect("fake tool directory should exist");
        let venv_python_template = dir.write_file(
            "venv-python",
            "#!/bin/sh\n\
             test \"$1\" = -m || exit 20\n\
             test \"$2\" = pip || exit 21\n\
             test \"$6\" = 'fixture-dependency<2' || exit 22\n\
             root=${0%/*}\n\
             /bin/printf 'installed\\n' > \"$root/../fixture-module\"\n\
             /bin/printf '#!/bin/sh\\ntest -f \"${0%%/*}/../fixture-module\"\\n' > \"$root/fixture-lsp\"\n\
             /bin/chmod 755 \"$root/fixture-lsp\"\n",
        );
        make_executable(&venv_python_template);
        let python = dir.write_file(
            "tools/python3",
            "#!/bin/sh\n\
             test \"$1\" = -m || exit 30\n\
             test \"$2\" = venv || exit 31\n\
             while [ \"$#\" -gt 1 ]; do shift; done\n\
             /bin/mkdir -p \"$1/bin\"\n\
             /bin/cp \"$FAKE_VENV_PYTHON\" \"$1/bin/python3\"\n\
             /bin/chmod 755 \"$1/bin/python3\"\n",
        );
        make_executable(&python);

        let package = pypi_package();
        let package_dir = state.package_dir(&package.name);
        let stale_launcher = package_dir.join("bin/fixture-lsp");
        fs::create_dir_all(stale_launcher.parent().expect("launcher has a parent"))
            .expect("stale prefix directory should exist");
        fs::write(&stale_launcher, b"#!/bin/sh\nexit 99\n")
            .expect("stale prefix launcher should be written");
        make_executable(&stale_launcher);

        let installed = with_env_vars(
            &[
                env_var("PATH", &tools),
                env_var("FAKE_VENV_PYTHON", &venv_python_template),
            ],
            || {
                resolve_or_install_program(&state, &package, "fixture-lsp")
                    .expect("PyPI package should install in a virtual environment")
            },
        );

        assert_eq!(installed, package_dir.join("local/bin/fixture-lsp"));
        assert!(package_dir.join("local").join(VENV_LAYOUT_MARKER).is_file());
        assert!(
            Command::new(&installed)
                .env_clear()
                .status()
                .expect("installed launcher should start")
                .success()
        );

        fs::remove_file(&python).expect("fake ambient Python should be removed");
        assert_eq!(
            resolve_or_install_program(&state, &package, "fixture-lsp")
                .expect("valid virtual environment should be reused"),
            installed
        );

        let mut changed_package = package;
        changed_package
            .source
            .extra_packages
            .push("another-dependency==1".to_string());
        let error = with_env_vars(&[env_var("PATH", "/nonexistent")], || {
            resolve_or_install_program(&state, &changed_package, "fixture-lsp")
                .expect_err("a changed dependency set should invalidate the cached environment")
        });
        assert!(error.contains("python3"), "unexpected error: {error}");
    }

    fn pypi_package() -> MasonPackage {
        MasonPackage {
            name: "fixture-pypi".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:pypi/fixture-package@1.2.3".to_string(),
                extra_packages: vec!["fixture-dependency<2".to_string()],
                asset: None,
                download: None,
                version_overrides: Vec::new(),
            },
            bin: BTreeMap::from([("fixture-lsp".to_string(), "pypi:fixture-lsp".to_string())]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("fixture_lsp".to_string()),
            },
        }
    }
}
