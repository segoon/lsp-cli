use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::process;

use super::{E2eContext, E2eOutput};

const DAEMON_CLEANUP_DEADLINE: Duration = Duration::from_secs(5);

impl Drop for E2eContext {
    fn drop(&mut self) {
        // Explicit finalization must not launch another command after proving ECHILD. A supervised
        // context also retains its roots if unwinding bypasses finalization; Drop cannot prove safety.
        if self.finalized || self.reaper.is_some() {
            return;
        }
        if let Err(diagnostic) = self.stop_daemons() {
            if std::thread::panicking() {
                eprintln!("E2E daemon cleanup failed:\n{diagnostic}");
            } else {
                panic!("E2E daemon cleanup failed:\n{diagnostic}");
            }
        }
    }
}

impl E2eContext {
    pub(super) fn stop_daemons(&self) -> Result<(), String> {
        let daemon_root = self.runtime_dir.join("lsp-cli");
        if !daemon_root.exists() {
            return Ok(());
        }

        // Detached daemons outlive command process groups, so the context must stop them explicitly.
        let mut command = self.command();
        command.args(["stop-all", "--debug"]);
        let cleanup = process::run(&mut command, DAEMON_CLEANUP_DEADLINE);
        let diagnostic = match cleanup {
            Ok(output) if output.status().success() => return Ok(()),
            Ok(output) => output.diagnostic(
                "E2E daemon cleanup exited unsuccessfully",
                &runtime_state(&self.runtime_dir),
            ),
            Err(failure) => failure.diagnostic(&runtime_state(&self.runtime_dir)),
        };
        Err(diagnostic)
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

    pub(crate) fn assert_stdout_contains(&self, expected: &str) {
        if !self.stdout_text().contains(expected) {
            panic!(
                "{}",
                self.diagnostic(&format!("stdout does not contain {expected:?}"))
            );
        }
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

    pub(super) fn diagnostic(&self, reason: &str) -> String {
        self.process
            .diagnostic(reason, &runtime_state(&self.runtime_dir))
    }

    pub(crate) fn try_json<T: DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_slice(self.process.stdout())
            .map_err(|error| self.diagnostic(&format!("stdout is not valid JSON: {error}")))
    }
}

pub(super) fn runtime_state(runtime_dir: &Path) -> String {
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
