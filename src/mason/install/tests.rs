use std::path::Path;

#[cfg(unix)]
use std::fs;

use super::{
    artifacts::parse_archive_file_spec, nuget_install_command, resolve_or_install_program,
};
#[cfg(unix)]
use crate::runtime_state::RuntimeState;
#[cfg(unix)]
use crate::test_support::{TestDir, env_var, make_executable, roslyn_package, with_env_vars};

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
