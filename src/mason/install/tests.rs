use std::io::{Cursor, Write};
use std::path::Path;

#[cfg(unix)]
use std::fs;

use super::{
    artifacts::{
        artifact_download_url, command_failure_detail, install_downloaded_artifact,
        parse_archive_file_spec,
    },
    golang_install_target, installer_command, nuget_install_command, resolve_or_install_program,
};
#[cfg(unix)]
use crate::runtime_state::RuntimeState;
#[cfg(unix)]
use crate::test_support::{
    TestDir, asm_lsp_package, cue_package, env_var, make_executable, roslyn_package, with_env_vars,
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
fn extracts_vsix_as_a_zip_archive() {
    let directory = tempfile::tempdir().expect("temporary directory should initialize");
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    archive
        .start_file(
            "extension/server/dist/server.js",
            zip::write::SimpleFileOptions::default(),
        )
        .expect("VSIX entry should initialize");
    archive
        .write_all(b"server")
        .expect("VSIX entry should be written");
    let bytes = archive.finish().expect("VSIX should finish").into_inner();

    install_downloaded_artifact(directory.path(), "server.vsix", &bytes)
        .expect("VSIX should extract");

    assert_eq!(
        fs::read(directory.path().join("extension/server/dist/server.js"))
            .expect("VSIX launcher should be extracted"),
        b"server"
    );
    assert!(!directory.path().join("server.vsix").exists());
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
fn installer_failure_detail_skips_leading_banner() {
    let stderr = "Welcome to the installer\n\nCould not execute required helper\n";

    assert_eq!(
        command_failure_detail(stderr),
        "Could not execute required helper"
    );
}

#[test]
fn builds_go_install_targets_with_and_without_subpaths() {
    assert_eq!(
        golang_install_target("cuelang.org/go", "v0.17.1", Some("cmd/cue")),
        "cuelang.org/go/cmd/cue@v0.17.1"
    );
    assert_eq!(
        golang_install_target("golang.org/x/tools/gopls", "v0.23.0", None),
        "golang.org/x/tools/gopls@v0.23.0"
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
fn go_installer_uses_subpath_and_rejects_traversal_before_invocation() {
    let dir = TestDir::new("mason-go-subpath");
    let bin_dir = dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("fake Go directory should exist");
    let invocation = dir.path().join("go-invocation");
    let go = dir.write_file(
        "bin/go",
        "#!/bin/sh\n\
         printf '%s' \"$2\" > \"$GO_INVOCATION\"\n\
         /bin/mkdir -p \"$GOBIN\"\n\
         : > \"$GOBIN/cue\"\n\
         /bin/chmod 755 \"$GOBIN/cue\"\n",
    );
    make_executable(&go);
    let valid_state = RuntimeState::new(dir.path().join("valid-state"));
    let invalid_state = RuntimeState::new(dir.path().join("invalid-state"));
    let valid_source = "pkg:golang/cuelang.org/go@v0.17.1#cmd/cue";
    let invalid_source = "pkg:golang/cuelang.org/go@v0.17.1#../cmd/cue";

    with_env_vars(
        &[
            env_var("PATH", &bin_dir),
            env_var("GO_INVOCATION", &invocation),
        ],
        || {
            resolve_or_install_program(&valid_state, &cue_package(valid_source), "cue")
                .expect("Go subpath package should install");
            assert_eq!(
                fs::read_to_string(&invocation).expect("Go invocation should be recorded"),
                "cuelang.org/go/cmd/cue@v0.17.1"
            );
            fs::remove_file(&invocation).expect("invocation marker should be reset");

            let error =
                resolve_or_install_program(&invalid_state, &cue_package(invalid_source), "cue")
                    .expect_err("traversing Go subpath should fail")
                    .to_string();
            assert!(error.contains(invalid_source));
            assert!(!invocation.exists(), "invalid source invoked Go");
        },
    );
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

#[test]
fn jetbrains_artifacts_use_the_official_download_endpoint() {
    let path = "/language-server/kotlin-server/263.6379.0/kotlin-server-263.6379.0.tar.gz";
    for (source, destination) in [
        (
            "https://download-cdn.jetbrains.com",
            "https://download.jetbrains.com",
        ),
        (
            "https://download.jetbrains.com",
            "https://download.jetbrains.com",
        ),
        ("https://example.com", "https://example.com"),
        (
            "https://download-cdn.jetbrains.com.example.com",
            "https://download-cdn.jetbrains.com.example.com",
        ),
        (
            "http://download-cdn.jetbrains.com",
            "http://download-cdn.jetbrains.com",
        ),
    ] {
        let actual =
            artifact_download_url(&format!("{source}{path}")).expect("artifact URL should parse");
        assert_eq!(actual.as_str(), format!("{destination}{path}"));
    }
}

#[test]
fn artifact_urls_preserve_signed_queries_and_report_invalid_urls() {
    let signed = "https://download-cdn.jetbrains.com/archive.tar.gz?Expires=123&Signature=a%2Bb&Key-Pair-Id=key";
    assert_eq!(
        artifact_download_url(signed)
            .expect("signed URL should parse")
            .as_str(),
        signed,
    );
    let error = artifact_download_url("not a URL")
        .expect_err("invalid URL should fail")
        .to_string();
    assert!(error.contains("invalid download URL not a URL"));
}
