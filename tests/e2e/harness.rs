use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use fs_extra::dir::CopyOptions;
use serde::de::DeserializeOwned;
use tempfile::TempDir;

use crate::process::{self, ProcessOutput};

#[path = "../../src/test_support/temp_root.rs"]
mod temp_root;

use self::temp_root::{test_temp_base, test_temp_root};

const DEFAULT_COMMAND_DEADLINE: Duration = Duration::from_secs(30);
const DAEMON_CLEANUP_DEADLINE: Duration = Duration::from_secs(5);

pub(crate) struct E2eContext {
    _sandbox: TempDir,
    _runtime_sandbox: TempDir,
    home: PathBuf,
    config_home: PathBuf,
    runtime_dir: PathBuf,
    workspace: PathBuf,
    bin_dir: PathBuf,
    build_dir: PathBuf,
    data_dir: PathBuf,
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
        let workspace = sandbox.path().join("workspace");
        let bin_dir = sandbox.path().join("bin");
        let build_dir = sandbox.path().join("build");
        let runtime_dir = runtime_sandbox.path().to_path_buf();

        for directory in [&home, &config_home, &workspace, &bin_dir, &build_dir] {
            fs::create_dir(directory)?;
        }

        Ok(Self {
            _sandbox: sandbox,
            _runtime_sandbox: runtime_sandbox,
            home,
            config_home,
            runtime_dir,
            workspace,
            bin_dir,
            build_dir,
            data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"),
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

    pub(crate) fn with_data_dir(mut self, data_dir: PathBuf) -> Self {
        self.data_dir = data_dir;
        self
    }

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
        command.args(args).current_dir(env!("CARGO_MANIFEST_DIR"));
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

    pub(crate) fn installed_data(&self) -> PathBuf {
        self.home.join(".local/share/lsp-cli/data")
    }

    pub(crate) fn run(&self, args: &[&str]) -> E2eOutput {
        self.run_with_deadline(args, DEFAULT_COMMAND_DEADLINE)
    }

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

    pub(crate) fn run_with_env(&self, args: &[&str], environment: &[(&str, &str)]) -> E2eOutput {
        let mut command = self.command();
        command.args(args).envs(environment.iter().copied());
        self.run_command(&mut command, DEFAULT_COMMAND_DEADLINE)
            .unwrap_or_else(|diagnostic| panic!("{diagnostic}"))
    }

    fn command(&self) -> Command {
        self.command_for(env!("CARGO_BIN_EXE_lsp-cli"))
    }

    fn command_for(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("CARGO_TARGET_DIR", &self.build_dir)
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("XDG_RUNTIME_DIR", &self.runtime_dir)
            .env("LSP_DATA", &self.data_dir)
            .env("PATH", &self.bin_dir)
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("TZ", "UTC")
            .current_dir(&self.workspace);
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

impl Drop for E2eContext {
    fn drop(&mut self) {
        let daemon_root = self.runtime_dir.join("lsp-cli");
        if !daemon_root.exists() {
            return;
        }

        // Detached daemons outlive command process groups, so the context must stop them explicitly.
        let mut command = self.command();
        command.args(["stop-all", "--debug"]);
        let cleanup = process::run(&mut command, DAEMON_CLEANUP_DEADLINE);
        let diagnostic = match cleanup {
            Ok(output) if output.status().success() => return,
            Ok(output) => output.diagnostic(
                "E2E daemon cleanup exited unsuccessfully",
                &runtime_state(&self.runtime_dir),
            ),
            Err(failure) => failure.diagnostic(&runtime_state(&self.runtime_dir)),
        };
        if std::thread::panicking() {
            eprintln!("E2E daemon cleanup failed:\n{diagnostic}");
        } else {
            panic!("E2E daemon cleanup failed:\n{diagnostic}");
        }
    }
}

impl E2eOutput {
    pub(crate) fn assert_success(&self) {
        self.ensure_success()
            .unwrap_or_else(|diagnostic| panic!("{diagnostic}"));
    }

    pub(crate) fn ensure_success(&self) -> Result<(), String> {
        if self.process.status().success() {
            Ok(())
        } else {
            Err(self.diagnostic("lsp-cli exited unsuccessfully"))
        }
    }

    pub(crate) fn stdout_text(&self) -> &str {
        std::str::from_utf8(self.process.stdout()).unwrap_or_else(|error| {
            panic!(
                "{}",
                self.diagnostic(&format!("stdout is not valid UTF-8: {error}"))
            )
        })
    }

    pub(crate) fn stderr_text(&self) -> &str {
        std::str::from_utf8(self.process.stderr()).unwrap_or_else(|error| {
            panic!(
                "{}",
                self.diagnostic(&format!("stderr is not valid UTF-8: {error}"))
            )
        })
    }

    pub(crate) fn json<T: DeserializeOwned>(&self) -> T {
        self.try_json()
            .unwrap_or_else(|diagnostic| panic!("{diagnostic}"))
    }

    fn diagnostic(&self, reason: &str) -> String {
        self.process
            .diagnostic(reason, &runtime_state(&self.runtime_dir))
    }

    pub(crate) fn try_json<T: DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_slice(self.process.stdout())
            .map_err(|error| self.diagnostic(&format!("stdout is not valid JSON: {error}")))
    }
}

fn runtime_state(runtime_dir: &std::path::Path) -> String {
    let daemon_root = runtime_dir.join("lsp-cli");
    let entries = match fs::read_dir(&daemon_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return format!("{} does not exist", daemon_root.display());
        }
        Err(error) => return format!("failed to read {}: {error}", daemon_root.display()),
    };
    let mut paths = entries
        .map(|entry| match entry {
            Ok(entry) => entry.path().display().to_string(),
            Err(error) => format!("<failed to read entry: {error}>"),
        })
        .collect::<Vec<_>>();
    paths.sort();
    if paths.is_empty() {
        format!("{} is empty", daemon_root.display())
    } else {
        paths.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    #[cfg(target_os = "linux")]
    use std::path::Path;
    #[cfg(target_os = "linux")]
    use std::time::Instant;

    use super::*;

    fn context() -> E2eContext {
        E2eContext::new().expect("E2E context should initialize")
    }

    fn run_shell(
        context: &E2eContext,
        script: &str,
        deadline: Duration,
    ) -> Result<E2eOutput, String> {
        context.run_test_program("/bin/sh", &["-c", script], deadline)
    }

    #[cfg(target_os = "linux")]
    fn assert_recorded_process_is_gone(pid_file: &Path) {
        let pid = fs::read_to_string(pid_file).expect("descendant PID should be recorded");
        let process = Path::new("/proc").join(&pid);
        let reaping_deadline = Instant::now() + Duration::from_secs(1);
        while process.exists() && Instant::now() < reaping_deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !process.exists(),
            "descendant process {pid} survived group cleanup"
        );
    }

    #[test]
    fn command_isolated_from_ambient_process_state() {
        let context = context();
        let command = context.command();
        let actual = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsString::from)))
            .collect::<BTreeMap<_, _>>();
        let expected = [
            (
                "CARGO_TARGET_DIR",
                context.build_dir.as_os_str().to_os_string(),
            ),
            ("HOME", context.home.as_os_str().to_os_string()),
            ("LANG", OsString::from("C")),
            ("LC_ALL", OsString::from("C")),
            ("LSP_DATA", context.data_dir.as_os_str().to_os_string()),
            ("PATH", context.bin_dir.as_os_str().to_os_string()),
            ("TZ", OsString::from("UTC")),
            (
                "XDG_CONFIG_HOME",
                context.config_home.as_os_str().to_os_string(),
            ),
            (
                "XDG_RUNTIME_DIR",
                context.runtime_dir.as_os_str().to_os_string(),
            ),
        ]
        .into_iter()
        .map(|(name, value)| (OsString::from(name), Some(value)))
        .collect::<BTreeMap<_, _>>();

        assert_eq!(actual, expected);
        assert_eq!(command.get_current_dir(), Some(context.workspace.as_path()));
        assert!(!context.runtime_dir.starts_with(context._sandbox.path()));
    }

    #[cfg(unix)]
    #[test]
    fn runtime_directory_has_room_for_daemon_socket_name() {
        use std::os::unix::net::UnixListener;

        let context = context();
        let daemon_root = context.runtime_dir.join("lsp-cli");
        fs::create_dir(&daemon_root).expect("daemon root should be created");
        let socket_path = daemon_root.join(format!("{}-{}.sock", "s".repeat(32), "f".repeat(24)));
        let _listener = UnixListener::bind(&socket_path).unwrap_or_else(|error| {
            panic!(
                "E2E runtime path {} cannot hold a daemon socket: {error}",
                socket_path.display()
            )
        });
    }

    #[test]
    fn captures_large_stdout_and_stderr_without_deadlock() {
        let context = context();
        let output = run_shell(
            &context,
            "i=0; while [ \"$i\" -lt 100000 ]; do printf o; printf e >&2; i=$((i + 1)); done",
            Duration::from_secs(5),
        )
        .expect("output fixture should finish");

        assert_eq!(output.process.stdout().len(), 100_000);
        assert_eq!(output.process.stderr().len(), 100_000);
    }

    #[test]
    fn deadline_kills_the_command_process_group() {
        let context = context();
        let pid_file = context.workspace.join("descendant.pid");
        let script = format!(
            "/bin/sleep 30 & child=$!; printf '%s' \"$child\" > {}; wait",
            pid_file.display()
        );
        let diagnostic = run_shell(&context, &script, Duration::from_millis(100))
            .err()
            .expect("stalled fixture should exceed its deadline");

        assert!(diagnostic.contains("process exceeded its deadline"));
        assert!(diagnostic.contains("process group killed and reaped"));
        #[cfg(target_os = "linux")]
        assert_recorded_process_is_gone(&pid_file);
    }

    #[test]
    fn deadline_includes_output_pipes_held_by_descendants() {
        let context = context();
        let pid_file = context.workspace.join("pipe-holder.pid");
        let script = format!(
            "/bin/sleep 30 & child=$!; printf '%s' \"$child\" > {}",
            pid_file.display()
        );
        let diagnostic = run_shell(&context, &script, Duration::from_millis(100))
            .err()
            .expect("inherited pipe should keep the process group beyond its deadline");

        assert!(diagnostic.contains("remained open after the command deadline"));
        assert!(diagnostic.contains("process group killed and reaped"));
        #[cfg(target_os = "linux")]
        assert_recorded_process_is_gone(&pid_file);
    }

    #[test]
    fn parses_json_output_into_requested_type() {
        let context = context();
        let output = run_shell(
            &context,
            "printf '%s' '{\"answer\":42}'",
            Duration::from_secs(1),
        )
        .expect("JSON fixture should finish");
        let value: serde_json::Value = output.json();

        assert_eq!(value, serde_json::json!({"answer": 42}));
    }

    #[test]
    fn invalid_json_reports_command_and_captured_output() {
        let context = context();
        let output = run_shell(&context, "printf not-json", Duration::from_secs(1))
            .expect("invalid JSON fixture should finish");
        let diagnostic = output
            .try_json::<serde_json::Value>()
            .expect_err("invalid JSON should be rejected");

        assert!(diagnostic.contains("stdout is not valid JSON"));
        assert!(diagnostic.contains("command: \"/bin/sh\" \"-c\""));
        assert!(diagnostic.contains("not-json"));
    }

    #[test]
    fn failed_command_diagnostic_includes_execution_context_and_output() {
        let context = context();
        let output = run_shell(
            &context,
            "printf stdout-marker; printf stderr-marker >&2; exit 7",
            Duration::from_secs(1),
        )
        .expect("failure fixture should finish");
        let diagnostic = output.diagnostic("fixture failed");

        assert!(diagnostic.contains("fixture failed"));
        assert!(diagnostic.contains("command: \"/bin/sh\" \"-c\""));
        assert!(diagnostic.contains(&format!(
            "working directory: {}",
            context.workspace.display()
        )));
        assert!(diagnostic.contains("status: exit status: 7"));
        assert!(diagnostic.contains("stdout-marker"));
        assert!(diagnostic.contains("stderr-marker"));
        assert!(diagnostic.contains("runtime state:"));
    }
}
