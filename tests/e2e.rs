#![expect(
    clippy::panic,
    reason = "E2E assertion helpers panic with captured process diagnostics."
)]

#[path = "e2e/catalog.rs"]
mod catalog;
#[path = "e2e/harness.rs"]
mod harness;
#[path = "e2e/manifest.rs"]
mod manifest;
#[path = "e2e/process.rs"]
mod process;
