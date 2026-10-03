#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "E2E assertions report fixture failures with captured process diagnostics."
)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;

mod case_files;
mod child_reaper;
mod dependencies;
mod failure_stage;
mod harness;
mod lsp_exchange;
mod manifest;
mod manifest_data;
mod process;
mod provisioning;
mod real_server_lifecycle;
mod real_server_support;
mod real_servers;
mod results;
mod runner;
mod runner_selection;

#[cfg(feature = "e2e-workflow-planner")]
pub mod workflow_plan;

#[cfg(test)]
mod catalog;
#[cfg(test)]
mod filesystem;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod lifecycle;
#[cfg(test)]
mod local_fixture;
#[cfg(test)]
mod queries;
#[cfg(test)]
mod update;

static LSP_CLI_BINARY: OnceLock<PathBuf> = OnceLock::new();

/// Run the real-server suite with the executable built by Cargo for this integration test.
pub fn run_suite(binary: &Path) -> ExitCode {
    initialize(binary);
    runner::main()
}

/// Run process-isolated child-supervision regressions on the executable's main thread.
pub fn run_reaper_regressions(binary: &Path) {
    initialize(binary);
    #[cfg(target_os = "linux")]
    child_reaper::regression::run();
}

fn initialize(binary: &Path) {
    LSP_CLI_BINARY
        .set(binary.to_path_buf())
        .expect("E2E entry point should initialize the executable once");
}

fn lsp_cli_binary() -> &'static Path {
    #[cfg(test)]
    LSP_CLI_BINARY.get_or_init(|| {
        // Workspace tests build lsp-cli alongside the library's unit-test executable in deps/.
        // Resolving from that executable also supports custom target directories and profiles.
        std::env::current_exe()
            .expect("unit-test executable path")
            .parent()
            .and_then(Path::parent)
            .expect("Cargo unit-test executable should be in the profile's deps directory")
            .join(format!("lsp-cli{}", std::env::consts::EXE_SUFFIX))
    });
    LSP_CLI_BINARY
        .get()
        .expect("E2E entry point should supply the lsp-cli executable")
}

/// Repository containing the E2E support package, fixtures, and server catalog.
pub fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("E2E support crate should be in tests/e2e")
}
