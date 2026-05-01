use crate::mason::registry::MasonPackage;
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use crate::suggest::SuggestedLanguage;
use serde::Serialize;
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedProgram {
    Direct(PathBuf),
    Wrapper(WrapperProgram),
}

impl ResolvedProgram {
    pub(crate) fn executable_path(&self) -> &Path {
        match self {
            Self::Direct(path) => path,
            Self::Wrapper(wrapper) => &wrapper.launcher_path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WrapperProgram {
    launcher_path: PathBuf,
    target_path: PathBuf,
    runtime: WrapperRuntime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WrapperRuntime {
    Python,
    Node,
    Dotnet,
    Java,
}

impl WrapperRuntime {
    fn command_name(self) -> &'static str {
        match self {
            Self::Python => "python3",
            Self::Node => "node",
            Self::Dotnet => "dotnet",
            Self::Java => "java",
        }
    }

    fn script_line(self, target_path: &Path) -> String {
        let target = shell_quote(target_path);
        match self {
            Self::Python => format!("exec python3 {target} \"$@\""),
            Self::Node => format!("exec node {target} \"$@\""),
            Self::Dotnet => format!("exec dotnet {target} \"$@\""),
            Self::Java => format!("exec java -jar {target} \"$@\""),
        }
    }
}

pub(crate) fn resolve_program(
    package: &MasonPackage,
    program: &str,
    state: &RuntimeState,
    context: &TemplateContext<'_>,
) -> Result<ResolvedProgram, String> {
    let target = package.bin.get(program).ok_or_else(|| {
        format!(
            "cannot install {} because Mason does not expose executable {program}",
            package.name
        )
    })?;
    let rendered = context.render(target);

    if let Some(relative) = rendered.strip_prefix("npm:") {
        return Ok(ResolvedProgram::Direct(
            state
                .package_dir(&package.name)
                .join("node_modules")
                .join(".bin")
                .join(relative),
        ));
    }

    if let Some(relative) = rendered.strip_prefix("pypi:") {
        let root = state.package_dir(&package.name);
        let primary = root.join("bin").join(relative);
        if primary.exists() {
            return Ok(ResolvedProgram::Direct(primary));
        }
        let alternate = root.join("local").join("bin").join(relative);
        if alternate.exists() {
            return Ok(ResolvedProgram::Direct(alternate));
        }
        return Ok(ResolvedProgram::Direct(alternate));
    }

    if let Some(relative) = rendered.strip_prefix("cargo:") {
        return Ok(ResolvedProgram::Direct(
            state.package_dir(&package.name).join("bin").join(relative),
        ));
    }

    if let Some(relative) = rendered.strip_prefix("golang:") {
        return Ok(ResolvedProgram::Direct(
            state.package_dir(&package.name).join("bin").join(relative),
        ));
    }

    if let Some(relative) = rendered.strip_prefix("python:") {
        return Ok(ResolvedProgram::Wrapper(WrapperProgram {
            launcher_path: state.bin_dir().join(program),
            target_path: join_relative_path(&state.package_dir(&package.name), relative)?,
            runtime: WrapperRuntime::Python,
        }));
    }

    if let Some(relative) = rendered.strip_prefix("node:") {
        return Ok(ResolvedProgram::Wrapper(WrapperProgram {
            launcher_path: state.bin_dir().join(program),
            target_path: join_relative_path(&state.package_dir(&package.name), relative)?,
            runtime: WrapperRuntime::Node,
        }));
    }

    if let Some(relative) = rendered.strip_prefix("dotnet:") {
        return Ok(ResolvedProgram::Wrapper(WrapperProgram {
            launcher_path: state.bin_dir().join(program),
            target_path: join_relative_path(&state.package_dir(&package.name), relative)?,
            runtime: WrapperRuntime::Dotnet,
        }));
    }

    if let Some(relative) = rendered.strip_prefix("java-jar:") {
        return Ok(ResolvedProgram::Wrapper(WrapperProgram {
            launcher_path: state.bin_dir().join(program),
            target_path: join_relative_path(&state.package_dir(&package.name), relative)?,
            runtime: WrapperRuntime::Java,
        }));
    }

    let relative = rendered.strip_prefix("exec:").unwrap_or(&rendered);
    Ok(ResolvedProgram::Direct(join_relative_path(
        &state.package_dir(&package.name),
        relative,
    )?))
}

pub(crate) fn is_resolved_program_runnable(program: &ResolvedProgram) -> bool {
    match program {
        ResolvedProgram::Direct(path) => is_command_runnable_path(path),
        ResolvedProgram::Wrapper(wrapper) => {
            is_command_runnable_path(&wrapper.launcher_path)
                && wrapper.target_path.is_file()
                && is_command_runnable(wrapper.runtime.command_name())
        }
    }
}

pub(crate) fn finalize_install(
    state: &RuntimeState,
    package: &MasonPackage,
    program: &str,
    resolved_program: &ResolvedProgram,
    context: &TemplateContext<'_>,
    failure_reason: &str,
) -> Result<PathBuf, String> {
    materialize_share(state, package, context)?;
    ensure_resolved_program(package, program, resolved_program)?;

    if !is_resolved_program_runnable(resolved_program) {
        return Err(format!(
            "cannot install {} because {failure_reason} {program} executable",
            package.name
        ));
    }

    write_receipt(state, package, resolved_program.executable_path())
}

pub(crate) fn rewrite_program(suggestion: &SuggestedLanguage, program: &Path) -> SuggestedLanguage {
    let mut resolved = suggestion.clone();
    resolved.command[0] = program.display().to_string();
    resolved
}

pub(crate) fn is_command_runnable(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return is_command_runnable_path(Path::new(program));
    }

    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path).any(|entry| is_command_runnable_path(&entry.join(program)))
}

pub(crate) fn join_relative_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    join_path_components(root, Path::new(relative).components())
}

fn ensure_resolved_program(
    package: &MasonPackage,
    program: &str,
    resolved_program: &ResolvedProgram,
) -> Result<(), String> {
    match resolved_program {
        ResolvedProgram::Direct(path) => ensure_executable(path),
        ResolvedProgram::Wrapper(wrapper) => {
            require_command(wrapper.runtime.command_name(), package, program)?;
            if !wrapper.target_path.is_file() {
                return Err(format!(
                    "cannot install {} because the wrapped launcher target {} was not created",
                    package.name,
                    wrapper.target_path.display()
                ));
            }
            write_wrapper_script(
                &wrapper.launcher_path,
                wrapper.runtime,
                &wrapper.target_path,
            )?;
            ensure_executable(&wrapper.launcher_path)
        }
    }
}

fn materialize_share(
    state: &RuntimeState,
    package: &MasonPackage,
    context: &TemplateContext<'_>,
) -> Result<(), String> {
    for (target, source) in &package.share {
        let rendered_target = context.render(target);
        let rendered_source = context.render(source);
        let target_is_dir = rendered_target.ends_with('/');
        let source_is_dir = rendered_source.ends_with('/');
        let share_path =
            join_relative_path(&state.share_dir(), rendered_target.trim_end_matches('/'))?;
        let package_path = join_relative_path(
            &state.package_dir(&package.name),
            rendered_source.trim_end_matches('/'),
        )?;

        if target_is_dir || source_is_dir {
            copy_directory_contents(&package_path, &share_path)?;
        } else {
            copy_file(&package_path, &share_path)?;
        }
    }

    Ok(())
}

fn write_wrapper_script(
    launcher_path: &Path,
    runtime: WrapperRuntime,
    target_path: &Path,
) -> Result<(), String> {
    let contents = format!("#!/bin/sh\n{}\n", runtime.script_line(target_path));
    write_file(launcher_path, contents.as_bytes())
}

fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    let bytes = fs::read(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
    write_file(target, &bytes)
}

fn copy_directory_contents(source: &Path, target: &Path) -> Result<(), String> {
    let metadata = fs::metadata(source)
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?;
    if !metadata.is_dir() {
        return Err(format!("expected directory at {}", source.display()));
    }

    fs::create_dir_all(target)
        .map_err(|error| format!("failed to create {}: {error}", target.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = entry
            .metadata()
            .map_err(|error| format!("failed to inspect {}: {error}", source_path.display()))?;
        if metadata.is_dir() {
            copy_directory_contents(&source_path, &target_path)?;
        } else if metadata.is_file() {
            copy_file(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn shell_quote(path: &Path) -> String {
    let value = path.display().to_string().replace('\'', "'\"'\"'");
    format!("'{value}'")
}

fn require_command(command: &str, package: &MasonPackage, program: &str) -> Result<(), String> {
    if is_command_runnable(command) {
        Ok(())
    } else {
        Err(format!(
            "cannot install {} because {} is not available in $PATH",
            suggestion_server_name(package, program),
            command
        ))
    }
}

fn suggestion_server_name(package: &MasonPackage, program: &str) -> String {
    if package.bin.contains_key(program) {
        program.to_string()
    } else {
        package.name.clone()
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err(format!(
            "failed to determine parent directory for {}",
            path.display()
        ));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn ensure_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(format!("failed to inspect {}: {error}", path.display())),
        };
        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        if mode & 0o111 == 0 {
            permissions.set_mode(mode | 0o755);
            fs::set_permissions(path, permissions).map_err(|error| {
                format!("failed to set permissions on {}: {error}", path.display())
            })?;
        }
    }

    Ok(())
}

fn is_command_runnable_path(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => is_executable(&metadata.permissions()),
        Ok(_) | Err(_) => false,
    }
}

#[cfg(unix)]
fn is_executable(permissions: &fs::Permissions) -> bool {
    permissions.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_permissions: &fs::Permissions) -> bool {
    true
}

fn join_path_components<'a>(
    root: &Path,
    components: impl IntoIterator<Item = Component<'a>>,
) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for component in components {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => {
                return Err(format!(
                    "downloaded package contains unsupported path outside {}",
                    root.display()
                ));
            }
        }
    }
    Ok(path)
}

#[derive(Debug, Serialize)]
struct InstallReceipt {
    package: String,
    source_id: String,
    executable: String,
}

fn write_receipt(
    state: &RuntimeState,
    package: &MasonPackage,
    executable_path: &Path,
) -> Result<PathBuf, String> {
    let receipt = InstallReceipt {
        package: package.name.clone(),
        source_id: package.source.id.clone(),
        executable: executable_path.display().to_string(),
    };
    let path = state.receipt_path(&package.name);
    let Some(parent) = path.parent() else {
        return Err(format!(
            "failed to determine parent directory for {}",
            path.display()
        ));
    };
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    fs::write(&path, bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(executable_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::{
        ResolvedProgram, WrapperRuntime, is_command_runnable, is_resolved_program_runnable,
        join_relative_path, resolve_program, rewrite_program,
    };
    use crate::mason::registry::{
        MasonAsset, MasonAssetBin, MasonDownload, MasonNeovim, MasonPackage, MasonSource, OneOrMany,
    };
    use crate::mason::template::TemplateContext;
    use crate::runtime_state::RuntimeState;
    use crate::suggest::SuggestedLanguage;
    use std::collections::BTreeMap;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

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
                "lsp-cli-mason-link-test-{}-{}",
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn pyright_package() -> MasonPackage {
        MasonPackage {
            name: "pyright".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:npm/pyright@1.1.409".to_string(),
                extra_packages: Vec::new(),
                asset: None,
                download: None,
                version_overrides: Vec::new(),
            },
            bin: BTreeMap::from([(
                "pyright-langserver".to_string(),
                "npm:pyright-langserver".to_string(),
            )]),
            share: BTreeMap::new(),
            neovim: MasonNeovim {
                lspconfig: Some("pyright".to_string()),
            },
        }
    }

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

    fn jdtls_package() -> MasonPackage {
        MasonPackage {
            name: "jdtls".to_string(),
            categories: vec!["LSP".to_string()],
            source: MasonSource {
                id: "pkg:generic/eclipse/eclipse.jdt.ls@v1.0.0".to_string(),
                extra_packages: Vec::new(),
                asset: None,
                download: Some(OneOrMany::Many(vec![MasonDownload {
                    target: Some(OneOrMany::One("linux".to_string())),
                    files: BTreeMap::from([(
                        "jdtls.tar.gz".to_string(),
                        "https://example.invalid/jdtls.tar.gz".to_string(),
                    )]),
                    bin: None,
                    config: Some("config_linux/".to_string()),
                    man: None,
                }])),
                version_overrides: Vec::new(),
            },
            bin: BTreeMap::from([("jdtls".to_string(), "python:bin/jdtls".to_string())]),
            share: BTreeMap::from([
                ("jdtls/plugins/".to_string(), "plugins/".to_string()),
                (
                    "jdtls/config/".to_string(),
                    "{{source.download.config}}".to_string(),
                ),
            ]),
            neovim: MasonNeovim {
                lspconfig: Some("jdtls".to_string()),
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

    fn suggestion() -> SuggestedLanguage {
        SuggestedLanguage {
            config_id: "pyright".to_string(),
            languages: vec!["python".to_string()],
            server: "pyright-langserver".to_string(),
            command: vec!["pyright-langserver".to_string(), "--stdio".to_string()],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
        }
    }

    fn resolve_program_path(
        package: &MasonPackage,
        program: &str,
        state: &RuntimeState,
        context: &TemplateContext<'_>,
    ) -> Result<PathBuf, String> {
        Ok(resolve_program(package, program, state, context)?
            .executable_path()
            .to_path_buf())
    }

    #[test]
    fn resolves_npm_program_path() {
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let _guard = env_lock().lock().expect("env lock should be available");
        let dir = TestDir::new();
        let state = RuntimeState::new(dir.path().join("state"));
        state.ensure_dirs().expect("state dirs should be created");
        let package_dir = state.package_dir("jdtls").join("bin");
        fs::create_dir_all(&package_dir).expect("package dir should be created");
        let target = package_dir.join("jdtls");
        fs::write(&target, b"print('ok')\n").expect("target should be written");
        let launcher = state.bin_dir().join("jdtls");
        fs::write(&launcher, b"#!/bin/sh\nexec python3 \"$@\"\n")
            .expect("launcher should be written");
        let mut permissions = fs::metadata(&launcher)
            .expect("metadata should be available")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&launcher, permissions).expect("permissions should be updated");

        let original_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", "/usr/bin") };
        let resolved =
            resolve_program(&jdtls_package(), "jdtls", &state, &TemplateContext::empty())
                .expect("wrapper should resolve");
        let runnable = is_resolved_program_runnable(&resolved);
        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert!(runnable);
    }

    #[test]
    fn materializes_share_mappings() {
        let dir = TestDir::new();
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
        let dir = TestDir::new();
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
        let dir = TestDir::new();
        let error =
            join_relative_path(dir.path(), "../escape").expect_err("parent path should fail");

        assert!(error.contains("outside"));
    }

    #[test]
    fn rewrites_detect_command_program() {
        let rewritten = rewrite_program(&suggestion(), &PathBuf::from("/tmp/pyright-langserver"));

        assert_eq!(
            rewritten.command,
            vec!["/tmp/pyright-langserver".to_string(), "--stdio".to_string(),]
        );
    }

    #[cfg(unix)]
    #[test]
    fn detects_runnable_command_on_path() {
        let _guard = env_lock().lock().expect("env lock should be available");
        let dir = TestDir::new();
        let executable = dir.path().join("pyright-langserver");
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("file should be written");
        let mut permissions = fs::metadata(&executable)
            .expect("metadata should be available")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).expect("permissions should be updated");

        let original_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", dir.path()) };
        let detected = is_command_runnable("pyright-langserver");
        match original_path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        assert!(detected);
    }
}
