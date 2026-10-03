use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use fs_extra::dir::CopyOptions;
use tempfile::TempDir;

use crate::child_reaper::ChildReaper;
use crate::process::{self, ProcessOutput};

#[path = "../../src/test_support/temp_root.rs"]
mod temp_root;

use self::temp_root::{test_temp_base, test_temp_root};

#[path = "harness/lifecycle_support.rs"]
mod lifecycle_support;
pub(crate) use lifecycle_support::SocketSnapshot;
#[path = "harness/cache_cleanup.rs"]
mod cache_cleanup;
#[path = "harness/environment.rs"]
mod environment;
#[path = "harness/failure_diagnostics.rs"]
mod failure_diagnostics;
#[path = "harness/process_state.rs"]
mod process_state;

use self::environment::{CARGO_HOME_ENV, INSTALL_PATH_ENV, RUSTUP_HOME_ENV, RUSTUP_TOOLCHAIN_ENV};
use self::process_state::runtime_state;

#[cfg(feature = "mock-tests")]
const DEFAULT_COMMAND_DEADLINE: Duration = Duration::from_secs(30);

pub(crate) struct E2eContext {
    reaper: Option<ChildReaper>,
    finalized: bool,
    _sandbox: TempDir,
    _runtime_sandbox: TempDir,
    home: PathBuf,
    config_home: PathBuf,
    runtime_dir: PathBuf,
    temp_dir: PathBuf,
    workspace: PathBuf,
    bin_dir: PathBuf,
    build_dir: PathBuf,
    data_dir: PathBuf,
    install_path: Option<std::ffi::OsString>,
    cargo_home: Option<std::ffi::OsString>,
    rustup_home: Option<std::ffi::OsString>,
    rustup_toolchain: Option<std::ffi::OsString>,
    // The staged `dotnet` apphost resolves its runtime via DOTNET_ROOT rather than PATH, so its
    // install root must be threaded through explicitly once `stage_host_program` resolves it.
    dotnet_root: RefCell<Option<PathBuf>>,
    // Prebuilt ruby-builder binaries carry a hard-coded RUNPATH and default $LOAD_PATH baked in
    // at build time, which don't match wherever this sandbox happens to stage them;
    // LD_LIBRARY_PATH/RUBYLIB are the overrides.
    ruby_env: RefCell<Option<RubyEnv>>,
}

struct RubyEnv {
    lib_dir: PathBuf,
    rubylib: std::ffi::OsString,
}

pub(crate) struct E2eOutput {
    process: ProcessOutput,
    runtime_dir: PathBuf,
}

impl E2eContext {
    pub(crate) fn new() -> io::Result<Self> {
        let test_temp_root = test_temp_root()?;
        fs::create_dir_all(&test_temp_root)?;
        let sandbox = tempfile::Builder::new()
            .prefix("lsp-cli-e2e-")
            .tempdir_in(test_temp_root)?;
        let test_temp_base = test_temp_base()?;
        fs::create_dir_all(&test_temp_base)?;
        // Keep this prefix and hierarchy short: daemon socket paths have a small OS limit.
        let runtime_sandbox = tempfile::Builder::new()
            .prefix("e-")
            .tempdir_in(test_temp_base)?;
        let home = sandbox.path().join("home");
        let config_home = sandbox.path().join("config");
        let temp_dir = sandbox.path().join("tmp");
        let workspace = sandbox.path().join("workspace");
        let bin_dir = sandbox.path().join("bin");
        let build_dir = sandbox.path().join("build");
        let runtime_dir = runtime_sandbox.path().to_path_buf();

        for directory in [
            &home,
            &config_home,
            &temp_dir,
            &workspace,
            &bin_dir,
            &build_dir,
        ] {
            fs::create_dir(directory)?;
        }

        Ok(Self {
            reaper: None,
            finalized: false,
            _sandbox: sandbox,
            _runtime_sandbox: runtime_sandbox,
            home,
            config_home,
            runtime_dir,
            temp_dir,
            workspace,
            bin_dir,
            build_dir,
            data_dir: crate::repository_root().join("data"),
            install_path: None,
            cargo_home: None,
            rustup_home: None,
            rustup_toolchain: None,
            dotnet_root: RefCell::new(None),
            ruby_env: RefCell::new(None),
        })
    }

    pub(crate) fn copy_project(&self, source: &Path) -> Result<(), String> {
        let options = CopyOptions::new().content_only(true);
        fs_extra::dir::copy(source, &self.workspace, &options)
            .map(|_copied_bytes| ())
            .map_err(|error| {
                format!(
                    "failed to copy E2E project {} into {}: {error}",
                    source.display(),
                    self.workspace.display()
                )
            })
    }

    #[cfg(feature = "mock-tests")]
    pub(crate) fn with_data_dir(mut self, data_dir: PathBuf) -> Self {
        self.data_dir = data_dir;
        self
    }

    #[cfg(any(test, feature = "mock-tests"))]
    pub(crate) fn stage_program(&self, name: &str, source: &Path) -> Result<(), String> {
        let source = source
            .canonicalize()
            .map_err(|error| format!("failed to resolve {}: {error}", source.display()))?;
        self.link_host_program(&source, &self.bin_dir.join(name))
            .map_err(|error| format!("failed to stage {} as {name:?}: {error}", source.display()))
    }

    pub(crate) fn stage_host_program(
        &self,
        name: &str,
        resolver: &[String],
        deadline: Duration,
    ) -> Result<(), String> {
        let (program, args) = resolver
            .split_first()
            .ok_or_else(|| format!("host program {name:?} has no resolver command"))?;
        let mut command = Command::new(program);
        command.args(args).current_dir(crate::repository_root());
        if let Some(path) = self.install_path.as_deref() {
            command.env("PATH", path);
        }
        let output = process::run(&mut command, deadline)
            .map_err(|failure| failure.diagnostic(&runtime_state(&self.runtime_dir)))?;
        if !output.status().success() {
            return Err(output.diagnostic(
                &format!("failed to resolve required host program {name:?}"),
                &runtime_state(&self.runtime_dir),
            ));
        }
        let stdout = std::str::from_utf8(output.stdout()).map_err(|error| {
            output.diagnostic(
                &format!("resolver output for host program {name:?} is not UTF-8: {error}"),
                &runtime_state(&self.runtime_dir),
            )
        })?;
        let paths = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let [resolved] = paths.as_slice() else {
            return Err(output.diagnostic(
                &format!(
                    "resolver for host program {name:?} must print exactly one non-empty path"
                ),
                &runtime_state(&self.runtime_dir),
            ));
        };
        let resolved = Path::new(resolved);
        if !resolved.is_file() {
            return Err(output.diagnostic(
                &format!(
                    "resolver for host program {name:?} returned {}, which is not a file",
                    resolved.display()
                ),
                &runtime_state(&self.runtime_dir),
            ));
        }
        let resolved = resolved.canonicalize().map_err(|error| {
            output.diagnostic(
                &format!(
                    "failed to resolve host program {name:?} path {}: {error}",
                    resolved.display()
                ),
                &runtime_state(&self.runtime_dir),
            )
        })?;

        if name == "dotnet"
            && let Some(root) = resolved.parent()
        {
            *self.dotnet_root.borrow_mut() = Some(root.to_path_buf());
        }
        if name == "ruby"
            && let Some(root) = resolved.parent().and_then(Path::parent)
        {
            let lib_dir = root.join("lib");
            let stdlib_root = lib_dir.join("ruby");
            let mut rubylib_entries = Vec::new();
            if let Ok(versions) = std::fs::read_dir(&stdlib_root) {
                for version_dir in versions.flatten().map(|entry| entry.path()) {
                    rubylib_entries.push(version_dir.clone());
                    if let Ok(archs) = std::fs::read_dir(&version_dir) {
                        rubylib_entries.extend(archs.flatten().map(|entry| entry.path()).filter(
                            |path| {
                                path.file_name()
                                    .and_then(|name| name.to_str())
                                    .is_some_and(|name| name.contains("linux"))
                            },
                        ));
                    }
                }
            }
            if lib_dir.is_dir() && !rubylib_entries.is_empty() {
                let rubylib = std::env::join_paths(&rubylib_entries)
                    .map_err(|error| format!("failed to build RUBYLIB for staged ruby: {error}"))?;
                *self.ruby_env.borrow_mut() = Some(RubyEnv { lib_dir, rubylib });
            }
        }

        self.link_host_program(&resolved, &self.bin_dir.join(name))
            .map_err(|error| {
                format!(
                    "failed to stage host program {name:?} from {}: {error}",
                    resolved.display()
                )
            })
    }

    #[cfg(unix)]
    fn link_host_program(&self, source: &Path, destination: &Path) -> io::Result<()> {
        match destination.canonicalize() {
            Ok(existing) if existing == source => return Ok(()),
            Ok(existing) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "{} already resolves to a different executable, {}",
                        destination.display(),
                        existing.display()
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if fs::symlink_metadata(destination).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} already exists and cannot be resolved",
                    destination.display()
                ),
            ));
        }
        std::os::unix::fs::symlink(source, destination)
    }

    #[cfg(not(unix))]
    fn link_host_program(&self, source: &Path, destination: &Path) -> io::Result<()> {
        fs::copy(source, destination).map(|_copied_bytes| ())
    }

    pub(crate) fn home(&self) -> &Path {
        &self.home
    }

    pub(crate) fn workspace(&self) -> &Path {
        &self.workspace
    }

    #[cfg(feature = "mock-tests")]
    pub(crate) fn installed_data(&self) -> PathBuf {
        self.home.join(".local/share/lsp-cli/data")
    }

    #[cfg(feature = "mock-tests")]
    pub(crate) fn run(&self, args: &[&str]) -> E2eOutput {
        self.run_with_deadline(args, DEFAULT_COMMAND_DEADLINE)
    }

    #[cfg(feature = "mock-tests")]
    pub(crate) fn run_with_deadline(&self, args: &[&str], deadline: Duration) -> E2eOutput {
        self.try_run_with_deadline(args, deadline)
            .unwrap_or_else(|diagnostic| panic!("{diagnostic}"))
    }

    pub(crate) fn try_run_with_deadline(
        &self,
        args: &[&str],
        deadline: Duration,
    ) -> Result<E2eOutput, String> {
        let mut command = self.command();
        command.args(args);
        self.run_command(&mut command, deadline)
    }

    #[cfg(feature = "mock-tests")]
    pub(crate) fn run_with_env(&self, args: &[&str], environment: &[(&str, &str)]) -> E2eOutput {
        let mut command = self.command();
        command.args(args).envs(environment.iter().copied());
        self.run_command(&mut command, DEFAULT_COMMAND_DEADLINE)
            .unwrap_or_else(|diagnostic| panic!("{diagnostic}"))
    }

    pub(crate) fn command(&self) -> Command {
        self.command_for(crate::lsp_cli_binary())
    }

    fn command_for(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("HOME", &self.home)
            // JVM home/temp properties do not follow HOME/TMPDIR. Isolate Gradle's daemon
            // registry as well, so a case cannot reuse a process outside its descendant tree.
            .env(
                "JAVA_TOOL_OPTIONS",
                format!(
                    "-Djava.io.tmpdir={} -Duser.home={}",
                    self.temp_dir.display(),
                    self.home.display()
                ),
            )
            .env("GRADLE_USER_HOME", self.home.join(".gradle"))
            .env("CARGO_TARGET_DIR", &self.build_dir)
            // Go makes module-cache directories read-only unless this flag is set, preventing the
            // isolated sandbox from being removed after a real-server case.
            .env("GOFLAGS", "-modcacherw")
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("XDG_RUNTIME_DIR", &self.runtime_dir)
            .env("LSP_DATA", &self.data_dir)
            .env("PATH", self.process_path())
            .env("TMPDIR", &self.temp_dir)
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("TZ", "UTC")
            .current_dir(&self.workspace);
        if let Some(root) = self.dotnet_root.borrow().as_deref() {
            command.env("DOTNET_ROOT", root);
        }
        if let Some(path) = self.install_path.as_deref() {
            command.env(INSTALL_PATH_ENV, path);
        }
        if let Some(path) = self.cargo_home.as_deref() {
            command.env(CARGO_HOME_ENV, path);
        }
        if let Some(path) = self.rustup_home.as_deref() {
            command.env(RUSTUP_HOME_ENV, path);
        }
        if let Some(toolchain) = self.rustup_toolchain.as_deref() {
            command.env(RUSTUP_TOOLCHAIN_ENV, toolchain);
        }
        if let Some(ruby_env) = self.ruby_env.borrow().as_ref() {
            command
                .env("LD_LIBRARY_PATH", &ruby_env.lib_dir)
                .env("RUBYLIB", &ruby_env.rubylib);
        }
        command
    }

    fn run_command(&self, command: &mut Command, deadline: Duration) -> Result<E2eOutput, String> {
        process::run(command, deadline)
            .map(|process| E2eOutput {
                process,
                runtime_dir: self.runtime_dir.clone(),
            })
            .map_err(|failure| failure.diagnostic(&runtime_state(&self.runtime_dir)))
    }

    #[cfg(test)]
    fn run_test_program(
        &self,
        program: impl AsRef<OsStr>,
        args: &[&str],
        deadline: Duration,
    ) -> Result<E2eOutput, String> {
        let mut command = self.command_for(program);
        command.args(args);
        self.run_command(&mut command, deadline)
    }
}

#[cfg(test)]
#[path = "harness/tests.rs"]
mod tests;
