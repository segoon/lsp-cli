#![allow(
    dead_code,
    unused_imports,
    reason = "This standalone test reuses E2E harness modules."
)]
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "Regression assertions report fixture failures."
)]

#[path = "e2e/case_files.rs"]
mod case_files;
#[path = "e2e/child_reaper.rs"]
mod child_reaper;
#[path = "e2e/dependencies.rs"]
mod dependencies;
#[path = "e2e/failure_stage.rs"]
mod failure_stage;
#[path = "e2e/harness.rs"]
mod harness;
#[path = "e2e/manifest.rs"]
mod manifest;
#[path = "e2e/manifest_data.rs"]
mod manifest_data;
#[path = "e2e/process.rs"]
mod process;
#[path = "e2e/results.rs"]
mod results;

pub(crate) fn repository_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn main() {
    #[cfg(target_os = "linux")]
    child_reaper::regression::run();
}
