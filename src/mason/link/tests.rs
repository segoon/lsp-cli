use super::{
    ResolvedProgram, WrapperRuntime, is_command_runnable, is_resolved_program_runnable,
    join_relative_path, resolve_program, rewrite_program,
};
use crate::error::Result;
use crate::mason::registry::{
    MasonAsset, MasonAssetBin, MasonDownload, MasonNeovim, MasonPackage, MasonSource, OneOrMany,
};
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use crate::test_support::{
    TestDir, env_var, jdtls_package, make_executable, pyright_package, suggested_language,
    with_env_vars,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn rust_analyzer_package() -> MasonPackage {
    MasonPackage {
        name: "rust-analyzer".to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: "pkg:github/rust-lang/rust-analyzer@2026-04-27".to_string(),
            extra_packages: Vec::new(),
            asset: Some(OneOrMany::Many(vec![MasonAsset {
                target: Some(OneOrMany::One("linux_x64_gnu".to_string())),
                file: OneOrMany::One("rust-analyzer-x86_64-unknown-linux-gnu.gz".to_string()),
                bin: Some(MasonAssetBin::One(
                    "rust-analyzer-x86_64-unknown-linux-gnu".to_string(),
                )),
                ext: None,
            }])),
            download: None,
            version_overrides: Vec::new(),
        },
        bin: BTreeMap::from([(
            "rust-analyzer".to_string(),
            "{{source.asset.bin}}".to_string(),
        )]),
        share: BTreeMap::new(),
        neovim: MasonNeovim {
            lspconfig: Some("rust_analyzer".to_string()),
        },
    }
}

fn generic_package() -> MasonPackage {
    MasonPackage {
        name: "bzl".to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: "pkg:generic/bzl@v1.4.22".to_string(),
            extra_packages: Vec::new(),
            asset: None,
            download: Some(OneOrMany::Many(vec![MasonDownload {
                target: Some(OneOrMany::One("linux_x64".to_string())),
                files: BTreeMap::from([(
                    "bzl".to_string(),
                    "https://example.invalid/linux_amd64/{{version}}/bzl".to_string(),
                )]),
                bin: Some("bzl".to_string()),
                config: None,
                man: None,
            }])),
            version_overrides: Vec::new(),
        },
        bin: BTreeMap::from([("bzl".to_string(), "{{source.download.bin}}".to_string())]),
        share: BTreeMap::new(),
        neovim: MasonNeovim {
            lspconfig: Some("bzl".to_string()),
        },
    }
}

fn ast_grep_package() -> MasonPackage {
    MasonPackage {
        name: "ast-grep".to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: "pkg:github/ast-grep/ast-grep@0.42.1".to_string(),
            extra_packages: Vec::new(),
            asset: Some(OneOrMany::Many(vec![MasonAsset {
                target: Some(OneOrMany::One("linux_x64_gnu".to_string())),
                file: OneOrMany::One("app-x86_64-unknown-linux-gnu.zip".to_string()),
                bin: None,
                ext: None,
            }])),
            download: None,
            version_overrides: Vec::new(),
        },
        bin: BTreeMap::from([(
            "ast-grep".to_string(),
            "ast-grep{{source.asset.ext}}".to_string(),
        )]),
        share: BTreeMap::new(),
        neovim: MasonNeovim {
            lspconfig: Some("ast_grep".to_string()),
        },
    }
}

fn quick_lint_js_package() -> MasonPackage {
    MasonPackage {
        name: "quick-lint-js".to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: "pkg:generic/quick-lint/quick-lint-js@3.2.0".to_string(),
            extra_packages: Vec::new(),
            asset: None,
            download: Some(OneOrMany::Many(vec![MasonDownload {
                target: Some(OneOrMany::One("linux_x64".to_string())),
                files: BTreeMap::from([(
                    "linux.tar.gz".to_string(),
                    "https://example.invalid/linux.tar.gz".to_string(),
                )]),
                bin: Some("quick-lint-js/bin/quick-lint-js".to_string()),
                config: None,
                man: Some("quick-lint-js/share/man/".to_string()),
            }])),
            version_overrides: Vec::new(),
        },
        bin: BTreeMap::from([(
            "quick-lint-js".to_string(),
            "{{source.download.bin}}".to_string(),
        )]),
        share: BTreeMap::from([("man/".to_string(), "{{source.download.man}}".to_string())]),
        neovim: MasonNeovim {
            lspconfig: Some("quick_lint_js".to_string()),
        },
    }
}

fn kcl_package() -> MasonPackage {
    MasonPackage {
        name: "kcl".to_string(),
        categories: vec!["LSP".to_string()],
        source: MasonSource {
            id: "pkg:github/kcl-lang/kcl@v0.11.2".to_string(),
            extra_packages: Vec::new(),
            asset: Some(OneOrMany::Many(vec![MasonAsset {
                target: Some(OneOrMany::One("linux_x64_gnu".to_string())),
                file: OneOrMany::One("kclvm-v0.11.2-linux-amd64.tar.gz".to_string()),
                bin: Some(MasonAssetBin::Many(BTreeMap::from([
                    ("kcl".to_string(), "exec:kclvm/bin/kclvm_cli".to_string()),
                    (
                        "kcl_language_server".to_string(),
                        "exec:kclvm/bin/kcl-language-server".to_string(),
                    ),
                ]))),
                ext: None,
            }])),
            download: None,
            version_overrides: Vec::new(),
        },
        bin: BTreeMap::from([(
            "kcl-language-server".to_string(),
            "{{source.asset.bin.kcl_language_server}}".to_string(),
        )]),
        share: BTreeMap::new(),
        neovim: MasonNeovim {
            lspconfig: Some("kcl".to_string()),
        },
    }
}

fn resolve_program_path(
    package: &MasonPackage,
    program: &str,
    state: &RuntimeState,
    context: &TemplateContext<'_>,
) -> Result<PathBuf> {
    Ok(resolve_program(package, program, state, context)?
        .executable_path()
        .to_path_buf())
}

#[test]
fn resolves_npm_program_path() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));

    assert_eq!(
        resolve_program_path(
            &pyright_package(),
            "pyright-langserver",
            &state,
            &TemplateContext::empty(),
        )
        .expect("managed path should exist"),
        state
            .package_dir("pyright")
            .join("node_modules")
            .join(".bin")
            .join("pyright-langserver")
    );
}

#[test]
fn resolves_github_template_program_path() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));
    let package = rust_analyzer_package();
    let context = TemplateContext {
        version: "2026-04-27",
        source_asset_bin: Some("rust-analyzer-x86_64-unknown-linux-gnu"),
        source_asset_file: Some("rust-analyzer-x86_64-unknown-linux-gnu.gz"),
        source_asset_ext: None,
        source_download_bin: None,
        source_download_config: None,
        source_download_man: None,
        source_asset_named_bins: BTreeMap::new(),
    };

    assert_eq!(
        resolve_program_path(&package, "rust-analyzer", &state, &context)
            .expect("github path should resolve"),
        state
            .package_dir("rust-analyzer")
            .join("rust-analyzer-x86_64-unknown-linux-gnu")
    );
}

#[test]
fn resolves_generic_template_program_path() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));
    let context = TemplateContext {
        version: "v1.4.22",
        source_asset_bin: None,
        source_asset_file: None,
        source_asset_ext: None,
        source_download_bin: Some("bzl"),
        source_download_config: None,
        source_download_man: None,
        source_asset_named_bins: BTreeMap::new(),
    };

    assert_eq!(
        resolve_program_path(&generic_package(), "bzl", &state, &context)
            .expect("generic path should resolve"),
        state.package_dir("bzl").join("bzl")
    );
}

#[test]
fn resolves_python_wrapper_program_path() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));
    let package = jdtls_package();

    assert_eq!(
        resolve_program_path(&package, "jdtls", &state, &TemplateContext::empty())
            .expect("wrapper path should resolve"),
        state.bin_dir().join("jdtls")
    );
    let resolved = resolve_program(&package, "jdtls", &state, &TemplateContext::empty())
        .expect("wrapper program should resolve");

    match resolved {
        ResolvedProgram::Wrapper(wrapper) => {
            assert_eq!(wrapper.launcher_path, state.bin_dir().join("jdtls"));
            assert_eq!(
                wrapper.target_path,
                state.package_dir("jdtls").join("bin").join("jdtls")
            );
            assert_eq!(wrapper.runtime, WrapperRuntime::Python);
        }
        ResolvedProgram::Direct(_) => panic!("expected wrapper program"),
    }
}

#[test]
fn resolves_github_asset_extension_template_to_empty_when_missing() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));
    let context = TemplateContext {
        version: "0.42.1",
        source_asset_bin: None,
        source_asset_file: Some("app-x86_64-unknown-linux-gnu.zip"),
        source_asset_ext: None,
        source_download_bin: None,
        source_download_config: None,
        source_download_man: None,
        source_asset_named_bins: BTreeMap::new(),
    };

    assert_eq!(
        resolve_program_path(&ast_grep_package(), "ast-grep", &state, &context)
            .expect("ast-grep path should resolve"),
        state.package_dir("ast-grep").join("ast-grep")
    );
}

#[test]
fn resolves_named_asset_bin_template() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));
    let context = TemplateContext {
        version: "v0.11.2",
        source_asset_bin: None,
        source_asset_file: Some("kclvm-v0.11.2-linux-amd64.tar.gz"),
        source_asset_ext: None,
        source_download_bin: None,
        source_download_config: None,
        source_download_man: None,
        source_asset_named_bins: BTreeMap::from([(
            "kcl_language_server".to_string(),
            "exec:kclvm/bin/kcl-language-server".to_string(),
        )]),
    };

    assert_eq!(
        resolve_program_path(&kcl_package(), "kcl-language-server", &state, &context)
            .expect("named asset bin path should resolve"),
        state
            .package_dir("kcl")
            .join("kclvm/bin/kcl-language-server")
    );
}

#[cfg(unix)]
#[test]
fn detects_runnable_wrapper_with_generated_launcher() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));
    state.ensure_dirs().expect("state dirs should be created");
    let package_dir = state.package_dir("jdtls").join("bin");
    fs::create_dir_all(&package_dir).expect("package dir should be created");
    let target = package_dir.join("jdtls");
    fs::write(&target, b"print('ok')\n").expect("target should be written");
    let launcher = state.bin_dir().join("jdtls");
    fs::write(&launcher, b"stub\n").expect("launcher should be written");
    make_executable(&launcher);
    let runtime_dir = dir.path().join("bin");
    fs::create_dir_all(&runtime_dir).expect("runtime dir should be created");
    let python = runtime_dir.join("python3");
    fs::write(&python, b"stub\n").expect("runtime should be written");
    make_executable(&python);

    let runnable = with_env_vars(
        &[env_var("PATH", runtime_dir.display().to_string())],
        || {
            let resolved =
                resolve_program(&jdtls_package(), "jdtls", &state, &TemplateContext::empty())
                    .expect("wrapper should resolve");
            is_resolved_program_runnable(&resolved)
        },
    );

    assert!(runnable);
}

#[test]
fn materializes_share_mappings() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));
    state.ensure_dirs().expect("state dirs should be created");
    let package = jdtls_package();
    let package_dir = state.package_dir("jdtls");
    fs::create_dir_all(package_dir.join("plugins")).expect("plugins dir should be created");
    fs::create_dir_all(package_dir.join("config_linux")).expect("config dir should be created");
    fs::write(package_dir.join("plugins").join("launcher.jar"), b"jar")
        .expect("plugin should be written");
    fs::write(package_dir.join("config_linux").join("config.ini"), b"cfg")
        .expect("config should be written");

    let context = TemplateContext {
        version: "v1.0.0",
        source_asset_bin: None,
        source_asset_file: None,
        source_asset_ext: None,
        source_download_bin: None,
        source_download_config: Some("config_linux/"),
        source_download_man: None,
        source_asset_named_bins: BTreeMap::new(),
    };
    super::materialize_share(&state, &package, &context).expect("share should materialize");

    assert!(
        state
            .share_dir()
            .join("jdtls/plugins/launcher.jar")
            .is_file()
    );
    assert!(state.share_dir().join("jdtls/config/config.ini").is_file());
}

#[test]
fn materializes_download_man_share_mapping() {
    let dir = TestDir::new("mason-link");
    let state = RuntimeState::new(dir.path().join("state"));
    state.ensure_dirs().expect("state dirs should be created");
    let package = quick_lint_js_package();
    let man_dir = state
        .package_dir("quick-lint-js")
        .join("quick-lint-js/share/man/man1");
    fs::create_dir_all(&man_dir).expect("man dir should be created");
    fs::write(man_dir.join("quick-lint-js.1"), b"man").expect("man page should be written");

    let context = TemplateContext {
        version: "3.2.0",
        source_asset_bin: None,
        source_asset_file: None,
        source_asset_ext: None,
        source_download_bin: Some("quick-lint-js/bin/quick-lint-js"),
        source_download_config: None,
        source_download_man: Some("quick-lint-js/share/man/"),
        source_asset_named_bins: BTreeMap::new(),
    };
    super::materialize_share(&state, &package, &context).expect("share should materialize");

    assert!(state.share_dir().join("man/man1/quick-lint-js.1").is_file());
}

#[test]
fn rejects_parent_directory_paths() {
    let dir = TestDir::new("mason-link");
    let error = join_relative_path(dir.path(), "../escape").expect_err("parent path should fail");

    assert!(error.contains("outside"));
}

#[test]
fn rewrites_detect_command_program() {
    let rewritten = rewrite_program(
        &suggested_language(
            "pyright-langserver",
            "pyright",
            "pyright-langserver",
            "python",
        ),
        &PathBuf::from("/tmp/pyright-langserver"),
    );

    assert_eq!(
        rewritten.command,
        vec!["/tmp/pyright-langserver".to_string(), "--stdio".to_string(),]
    );
}

#[cfg(unix)]
#[test]
fn detects_runnable_command_on_path() {
    let dir = TestDir::new("mason-link");
    let executable = dir.path().join("pyright-langserver");
    fs::write(&executable, b"stub\n").expect("file should be written");
    make_executable(&executable);

    let detected = with_env_vars(&[env_var("PATH", dir.path())], || {
        is_command_runnable("pyright-langserver")
    });

    assert!(detected);
}
