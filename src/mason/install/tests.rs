use std::path::Path;

#[cfg(unix)]
use std::fs;

use super::{
    artifacts::{command_failure_detail, parse_archive_file_spec},
    installer_command, nuget_install_command, pypi_install_command, resolve_or_install_program,
};
#[cfg(unix)]
use crate::runtime_state::RuntimeState;
#[cfg(unix)]
use crate::test_support::{
    TestDir, asm_lsp_package, env_var, make_executable, roslyn_package, with_env_vars,
};

#[test]
fn parses_archive_file_spec() {
    assert_eq!(
        parse_archive_file_spec("lua-language-server-3.18.2-linux-x64.tar.gz:libexec/"),
        (
            "lua-language-server-3.18.2-linux-x64.tar.gz",
            Some("libexec")
        )
    );
    assert_eq!(
        parse_archive_file_spec("clangd-linux-22.1.0.zip"),
        ("clangd-linux-22.1.0.zip", None)
    );
}

#[test]
fn builds_exact_nuget_tool_install_command() {
    let command = nuget_install_command(
        "roslyn-language-server",
        "5.11.0-1.26380.4",
        Path::new("managed/bin"),
    );

    assert_eq!(command.get_program(), "dotnet");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "tool",
            "install",
            "roslyn-language-server",
            "--tool-path",
            "managed/bin",
            "--version",
            "5.11.0-1.26380.4",
        ]
    );
}

#[test]
fn isolates_pypi_install_from_ambient_packages() {
    let command = pypi_install_command(
        "python-lsp-server",
        "1.15.0",
        &["all".to_string()],
        Path::new("managed/python"),
    );

    assert_eq!(command.get_program(), "python3");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "--ignore-installed",
            "--prefix",
            "managed/python",
            "python-lsp-server[all]==1.15.0",
        ]
    );
}

#[test]
fn installer_failure_detail_skips_leading_banner() {
    let stderr = "Welcome to the installer\n\nCould not execute required helper\n";

    assert_eq!(
        command_failure_detail(stderr),
        "Could not execute required helper"
    );
}

#[cfg(unix)]
#[test]
fn installs_and_caches_nuget_tool_with_a_receipt() {
    let dir = TestDir::new("mason-nuget-install");
    let state = RuntimeState::new(dir.path().join("state"));
    let bin_dir = dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("fake program directory should exist");
    let dotnet = dir.write_file(
        "bin/dotnet",
        "#!/bin/sh\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"--tool-path\" ]; then\n    shift\n    tool_path=$1\n  fi\n  shift\ndone\n/bin/mkdir -p \"$tool_path\"\n: > \"$tool_path/roslyn-language-server\"\n/bin/chmod 755 \"$tool_path/roslyn-language-server\"\n",
    );
    make_executable(&dotnet);

    let installed = with_env_vars(&[env_var("PATH", &bin_dir)], || {
        resolve_or_install_program(&state, &roslyn_package(), "roslyn-language-server")
            .expect("NuGet tool should install")
    });

    assert_eq!(
        installed,
        state
            .package_dir("roslyn-language-server")
            .join("bin/roslyn-language-server")
    );
    let receipt = fs::read_to_string(state.receipt_path("roslyn-language-server"))
        .expect("install receipt should exist");
    assert!(receipt.contains("pkg:nuget/roslyn-language-server@5.11.0-1.26380.4"));

    let cached = resolve_or_install_program(&state, &roslyn_package(), "roslyn-language-server")
        .expect("installed NuGet tool should be reusable from cache");
    assert_eq!(cached, installed);
}

#[cfg(unix)]
#[test]
fn cargo_installer_inherits_toolchain_state_with_an_isolated_home() {
    let dir = TestDir::new("mason-cargo-rustup-state");
    let state = RuntimeState::new(dir.path().join("state"));
    let bin_dir = dir.path().join("bin");
    let isolated_home = dir.path().join("isolated-home");
    let cargo_home = dir.path().join("cargo-home");
    let rustup_home = dir.path().join("rustup-home");
    for path in [&bin_dir, &isolated_home, &cargo_home, &rustup_home] {
        fs::create_dir_all(path).expect("fake toolchain directory should exist");
    }
    fs::create_dir(cargo_home.join("registry-marker"))
        .expect("fake Cargo state marker should be created");
    fs::write(
        rustup_home.join("settings.toml"),
        b"default_toolchain = 'stable'\n",
    )
    .expect("fake rustup settings should be written");
    let cargo = dir.write_file(
        "bin/cargo",
        "#!/bin/sh\n\
         test -f \"$RUSTUP_HOME/settings.toml\" || exit 20\n\
         test \"$RUSTUP_TOOLCHAIN\" = stable || exit 21\n\
         test -d \"$CARGO_HOME/registry-marker\" || exit 22\n\
         while [ \"$#\" -gt 0 ]; do\n\
           if [ \"$1\" = --root ]; then shift; root=$1; fi\n\
           shift\n\
         done\n\
         /bin/mkdir -p \"$root/bin\"\n\
         : > \"$root/bin/asm-lsp\"\n\
         /bin/chmod 755 \"$root/bin/asm-lsp\"\n",
    );
    make_executable(&cargo);

    let installed = with_env_vars(
        &[
            env_var("PATH", &bin_dir),
            env_var("HOME", &isolated_home),
            env_var("CARGO_HOME", "wrong-cargo-home"),
            env_var("RUSTUP_HOME", "wrong-rustup-home"),
            env_var("RUSTUP_TOOLCHAIN", "wrong-toolchain"),
            env_var("LSP_CLI_INSTALL_CARGO_HOME", &cargo_home),
            env_var("LSP_CLI_INSTALL_RUSTUP_HOME", &rustup_home),
            env_var("LSP_CLI_INSTALL_RUSTUP_TOOLCHAIN", "stable"),
        ],
        || {
            resolve_or_install_program(&state, &asm_lsp_package(), "asm-lsp")
                .expect("Cargo package should install with forwarded rustup state")
        },
    );

    assert_eq!(installed, state.package_dir("asm-lsp").join("bin/asm-lsp"));
}

#[cfg(unix)]
#[test]
fn installer_command_scopes_toolchain_environment_to_cargo() {
    let dir = TestDir::new("mason-installer-path");
    let path = dir.path().join("toolchain");
    let cargo_home = dir.path().join("cargo-home");

    let (cargo, npm) = with_env_vars(
        &[
            env_var("LSP_CLI_INSTALL_PATH", &path),
            env_var("LSP_CLI_INSTALL_CARGO_HOME", &cargo_home),
        ],
        || (installer_command("cargo"), installer_command("npm")),
    );
    let cargo_environment = cargo
        .get_envs()
        .map(|(name, value)| (name.to_os_string(), value.map(std::ffi::OsString::from)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let npm_environment = npm
        .get_envs()
        .map(|(name, value)| (name.to_os_string(), value.map(std::ffi::OsString::from)))
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(
        cargo_environment[std::ffi::OsStr::new("PATH")],
        Some(path.into_os_string())
    );
    assert_eq!(
        cargo_environment[std::ffi::OsStr::new("CARGO_HOME")],
        Some(cargo_home.into_os_string())
    );
    assert!(!npm_environment.contains_key(std::ffi::OsStr::new("CARGO_HOME")));
}
