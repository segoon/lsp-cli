#![expect(
    clippy::panic,
    clippy::expect_used,
    reason = "E2E assertions include captured process diagnostics."
)]
#![allow(
    dead_code,
    unused_imports,
    reason = "The runner reuses modules that also contain harness-only test helpers."
)]

#[path = "e2e/case_files.rs"]
mod case_files;
#[path = "e2e/dependencies.rs"]
mod dependencies;
#[path = "e2e/harness.rs"]
mod harness;
#[path = "e2e/lsp_exchange.rs"]
mod lsp_exchange;
#[path = "e2e/manifest.rs"]
mod manifest;
#[path = "e2e/manifest_data.rs"]
mod manifest_data;
#[path = "e2e/process.rs"]
mod process;
#[path = "e2e/provisioning.rs"]
mod provisioning;
#[path = "e2e/real_server_lifecycle.rs"]
mod real_server_lifecycle;
#[path = "e2e/real_server_support.rs"]
mod real_server_support;
#[path = "e2e/real_servers.rs"]
mod real_servers;
#[path = "e2e/results.rs"]
mod results;
#[path = "e2e/runner_selection.rs"]
mod runner_selection;

use std::path::Path;
use std::process::ExitCode;

use dependencies::ManagedDependencies;
use manifest::Manifest;
use real_server_support::RunReport;
use runner_selection::{Phase, Selection};

pub(crate) fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn run(selection: &Selection) -> Result<(), String> {
    let repository = repository_root();
    let manifest = Manifest::load_validated(repository)?;
    selection.validate(&manifest)?;
    if !manifest.supports_current_platform() {
        return Err(format!(
            "E2E tests require {}; current platform is {}/{}",
            manifest.platform_label(),
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }
    if let Some(label) = selection.case.as_deref()
        && let Some(reason) = manifest.exclusion_reason(label)
    {
        eprintln!("E2E case {label}: reviewed exclusion: {reason}");
    }

    let selected_behavior = match selection.phase {
        Phase::Provision => 0,
        Phase::Smoke => selected_smoke_count(&manifest, selection),
        Phase::Lifecycle => selected_lifecycle_count(&manifest, selection),
        Phase::All => {
            selected_smoke_count(&manifest, selection)
                + selected_lifecycle_count(&manifest, selection)
        }
    };
    if selection.phase != Phase::Provision
        && (selection.case.is_some() || selection.server.is_some())
        && selected_behavior == 0
    {
        return Err("the selected E2E scope has no executable behavior tests".to_string());
    }
    if selection.phase == Phase::Provision
        && (selection.case.is_some() || selection.server.is_some())
        && manifest
            .server_provisioning_cases()
            .all(|case| !selection.includes_server(&manifest, case.server_id()))
    {
        return Err("the selected E2E scope has no executable provisioning test".to_string());
    }

    let dependencies = ManagedDependencies::prepare()?;
    let excluded = if selection.phase == Phase::Provision {
        0
    } else {
        manifest.excluded_behavior_count(|label, server, smoke| {
            selection.includes_pair(label, server, smoke)
        })
    };
    let mut report = RunReport::default();
    if matches!(selection.phase, Phase::All | Phase::Provision) {
        report.merge(provisioning::run_cases(
            &manifest,
            repository,
            &dependencies,
            |server| selection.includes_server(&manifest, server),
        ));
    }
    let mut behavior = RunReport::default();
    if matches!(selection.phase, Phase::All | Phase::Smoke) {
        behavior.merge(real_servers::run_cases(
            &manifest,
            repository,
            &dependencies,
            |label, server, smoke| selection.includes_pair(label, server, smoke),
        ));
    }
    if matches!(selection.phase, Phase::All | Phase::Lifecycle) {
        behavior.merge(real_server_lifecycle::run_cases(
            &manifest,
            repository,
            &dependencies,
            |label, server, smoke| selection.includes_pair(label, server, smoke),
        ));
    }
    report.merge(behavior);
    let passed = report.planned - report.failures.len();
    eprintln!(
        "E2E summary: planned {}, executed {}, passed {}, failed {}, excluded {}",
        report.planned + excluded,
        report.planned,
        passed,
        report.failures.len(),
        excluded
    );
    if report.failures.is_empty() {
        Ok(())
    } else {
        Err(format!("E2E failures:\n{}", report.failures.join("\n\n")))
    }
}

fn selected_smoke_count(manifest: &Manifest, selection: &Selection) -> usize {
    manifest
        .real_server_smoke_cases()
        .filter(|case| selection.includes_pair(&case.label(), case.server_id(), case.is_smoke()))
        .count()
        + manifest
            .real_server_capabilities_cases()
            .filter(|case| {
                selection.includes_pair(&case.label(), case.server_id(), case.is_smoke())
            })
            .count()
}

fn selected_lifecycle_count(manifest: &Manifest, selection: &Selection) -> usize {
    manifest
        .real_server_lifecycle_cases()
        .filter(|case| selection.includes_pair(&case.label(), case.server_id(), case.is_smoke()))
        .count()
}

fn main() -> ExitCode {
    let mut args = std::env::args();
    let _program = args.next();
    if args.len() == 0 {
        return ExitCode::SUCCESS;
    }
    let result = Selection::parse(args).and_then(|selection| run(&selection));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("E2E runner error: {error}");
            ExitCode::FAILURE
        }
    }
}
