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

#[cfg(feature = "mock-tests")]
pub mod mock;

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
    // Libtest runs mock cases concurrently; every case must supply the same Cargo-built binary.
    assert_eq!(
        LSP_CLI_BINARY.get_or_init(|| binary.to_path_buf()),
        binary,
        "E2E cases must use the same lsp-cli executable"
    );
}

fn lsp_cli_binary() -> &'static Path {
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
