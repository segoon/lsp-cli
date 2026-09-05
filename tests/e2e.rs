#![expect(
    clippy::panic,
    reason = "E2E assertion helpers panic with captured process diagnostics."
)]
#![expect(
    clippy::expect_used,
    reason = "E2E fixtures and assertions fail immediately with contextual expectation messages."
)]

#[path = "e2e/catalog.rs"]
mod catalog;
#[path = "e2e/filesystem.rs"]
mod filesystem;
#[path = "e2e/fixture.rs"]
mod fixture;
#[path = "e2e/harness.rs"]
mod harness;
#[path = "e2e/lifecycle.rs"]
mod lifecycle;
#[path = "e2e/local_fixture.rs"]
mod local_fixture;
#[path = "e2e/manifest.rs"]
mod manifest;
#[path = "e2e/process.rs"]
mod process;
#[path = "e2e/queries.rs"]
mod queries;
#[path = "e2e/real_servers.rs"]
mod real_servers;
#[path = "e2e/update.rs"]
mod update;

use std::path::Path;

pub(crate) fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
