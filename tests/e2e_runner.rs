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
#[path = "e2e/failure_stage.rs"]
mod failure_stage;
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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::ExitCode;

use dependencies::ManagedDependencies;
use manifest::Manifest;
use real_server_support::RunReport;
use results::CaseKind;
use runner_selection::{Action, Phase, Selection};

pub(crate) fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn run(selection: &Selection) -> Result<(), String> {
    let repository = repository_root();
    let manifest = load_manifest(selection)?;
    validate_scope(&manifest, selection)?;
    if let Some(label) = selection.case.as_deref()
        && let Some(reason) = manifest.exclusion_reason(label)
    {
        eprintln!("E2E case {label}: reviewed exclusion: {reason}");
    }

    let dependencies = ManagedDependencies::prepare()?;
    let excluded = excluded_count(&manifest, selection);
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
    if !selection.quiet_summary {
        print_summary(&report, excluded, passed);
    }
    if report.failures.is_empty() || selection.defer_failures {
        Ok(())
    } else {
        Err(format!("E2E failures:\n{}", report.failures.join("\n\n")))
    }
}

fn load_manifest(selection: &Selection) -> Result<Manifest, String> {
    let manifest = Manifest::load_validated(repository_root())?;
    selection.validate(&manifest)?;
    if !manifest.supports_current_platform() {
        return Err(format!(
            "E2E tests require {}; current platform is {}/{}",
            manifest.platform_label(),
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }
    Ok(manifest)
}

fn validate_scope(manifest: &Manifest, selection: &Selection) -> Result<(), String> {
    let selected_behavior = match selection.phase {
        Phase::Provision => 0,
        Phase::Smoke => selected_smoke_count(manifest, selection),
        Phase::Lifecycle => selected_lifecycle_count(manifest, selection),
        Phase::All => {
            selected_smoke_count(manifest, selection)
                + selected_lifecycle_count(manifest, selection)
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
            .all(|case| !selection.includes_server(manifest, case.server_id()))
    {
        return Err("the selected E2E scope has no executable provisioning test".to_string());
    }
    Ok(())
}

fn list_work(selection: &Selection) -> Result<(), String> {
    if selection.phase == Phase::All {
        return Err("--list-work requires an explicit E2E phase".to_string());
    }
    let manifest = load_manifest(selection)?;
    let work: BTreeSet<String> = match selection.phase {
        Phase::Provision => manifest
            .server_provisioning_cases()
            .filter(|case| selection.includes_server(&manifest, case.server_id()))
            .map(|case| case.server_id().to_string())
            .collect(),
        Phase::Smoke => manifest
            .real_server_smoke_cases()
            .filter(|case| {
                selection.includes_pair(&case.label(), case.server_id(), case.is_smoke())
            })
            .map(|case| case.label())
            .chain(
                manifest
                    .real_server_capabilities_cases()
                    .filter(|case| {
                        selection.includes_pair(&case.label(), case.server_id(), case.is_smoke())
                    })
                    .map(|case| case.label()),
            )
            .collect(),
        Phase::Lifecycle => manifest
            .real_server_lifecycle_cases()
            .filter(|case| {
                selection.includes_pair(&case.label(), case.server_id(), case.is_smoke())
            })
            .map(|case| case.label())
            .collect(),
        Phase::All => return Err("--list-work requires an explicit E2E phase".to_string()),
    };
    for item in work {
        println!("{item}");
    }
    Ok(())
}

fn merge_results(selection: &Selection, directory: &Path) -> Result<(), String> {
    let manifest = load_manifest(selection)?;
    validate_scope(&manifest, selection)?;
    let expected = expected_results(&manifest, selection);
    let merged = results::merge_shards(directory, &expected)?;
    let excluded = excluded_count(&manifest, selection);
    let selected_keys = expected
        .iter()
        .map(|(kind, id)| format!("{}/{id}", case_kind_key(*kind)))
        .collect::<BTreeSet<_>>();
    let expected_failures = manifest
        .expected_failures()
        .filter(|failure| selected_keys.contains(failure.case()))
        .map(|failure| (failure.case(), failure))
        .collect::<BTreeMap<_, _>>();
    let actual_failures = merged
        .failures
        .iter()
        .map(|failure| failure.key.as_str())
        .collect::<BTreeSet<_>>();
    let mut matched_expected_failures = BTreeSet::new();
    let failures = merged
        .failures
        .iter()
        .filter_map(
            |failure| match expected_failures.get(failure.key.as_str()) {
                Some(expected)
                    if expected.matches(
                        failure.stage,
                        !failure.additional_stages.is_empty(),
                        &failure.diagnostic,
                    ) =>
                {
                    matched_expected_failures.insert(failure.key.as_str());
                    None
                }
                Some(expected) => Some(format!(
                    "E2E failure {} did not match its expected {}:\n{}",
                    failure.key,
                    expected.expectation(),
                    failure.diagnostic
                )),
                None => Some(failure.diagnostic.clone()),
            },
        )
        .collect::<Vec<_>>();
    let expected_keys = expected_failures.keys().copied().collect::<BTreeSet<_>>();
    let expected_passes = expected_keys
        .difference(&actual_failures)
        .copied()
        .collect::<Vec<_>>();
    let expected_failure_count = matched_expected_failures.len();
    let report = RunReport {
        planned: merged.planned,
        failures,
    };
    let passed = merged.passed.len();
    print_summary(&report, excluded, passed);
    eprintln!(
        "E2E expected failures: {expected_failure_count}; expected-failure cases passed: {}",
        expected_passes.len()
    );
    if !expected_passes.is_empty() {
        eprintln!(
            "E2E expected-failure cases that passed and may be ready for review:\n{}",
            expected_passes.join("\n")
        );
    }
    if report.failures.is_empty() {
        Ok(())
    } else {
        Err(format!("E2E failures:\n{}", report.failures.join("\n\n")))
    }
}

fn case_kind_key(kind: CaseKind) -> &'static str {
    match kind {
        CaseKind::Smoke => "smoke",
        CaseKind::Capabilities => "capabilities",
        CaseKind::Lifecycle => "lifecycle",
        CaseKind::Provisioning => "provisioning",
    }
}

fn expected_results(manifest: &Manifest, selection: &Selection) -> Vec<(CaseKind, String)> {
    let mut expected = Vec::new();
    if matches!(selection.phase, Phase::All | Phase::Provision) {
        expected.extend(
            manifest
                .server_provisioning_cases()
                .filter(|case| selection.includes_server(manifest, case.server_id()))
                .map(|case| (CaseKind::Provisioning, case.server_id().to_string())),
        );
    }
    if matches!(selection.phase, Phase::All | Phase::Smoke) {
        expected.extend(
            manifest
                .real_server_smoke_cases()
                .filter(|case| {
                    selection.includes_pair(&case.label(), case.server_id(), case.is_smoke())
                })
                .map(|case| (CaseKind::Smoke, case.label())),
        );
        expected.extend(
            manifest
                .real_server_capabilities_cases()
                .filter(|case| {
                    selection.includes_pair(&case.label(), case.server_id(), case.is_smoke())
                })
                .map(|case| (CaseKind::Capabilities, case.label())),
        );
    }
    if matches!(selection.phase, Phase::All | Phase::Lifecycle) {
        expected.extend(
            manifest
                .real_server_lifecycle_cases()
                .filter(|case| {
                    selection.includes_pair(&case.label(), case.server_id(), case.is_smoke())
                })
                .map(|case| (CaseKind::Lifecycle, case.label())),
        );
    }
    expected.sort();
    expected
}

fn excluded_count(manifest: &Manifest, selection: &Selection) -> usize {
    if selection.phase == Phase::Provision {
        0
    } else {
        manifest.excluded_behavior_count(|label, server, smoke| {
            selection.includes_pair(label, server, smoke)
        })
    }
}

fn print_summary(report: &RunReport, excluded: usize, passed: usize) {
    eprintln!(
        "E2E summary: planned {}, executed {}, passed {}, failed {}, excluded {}",
        report.planned + excluded,
        report.planned,
        passed,
        report.failures.len(),
        excluded
    );
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

fn render_servers_doc() -> Result<(), String> {
    let document =
        Manifest::load_validated(repository_root())?.render_server_docs(repository_root())?;
    print!("{document}");
    Ok(())
}

fn main() -> ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return ExitCode::SUCCESS;
    }
    let result = if args == ["--render-servers-doc"] {
        render_servers_doc()
    } else {
        Selection::parse(args.into_iter()).and_then(|selection| match &selection.action {
            Action::Run => run(&selection),
            Action::ListWork => list_work(&selection),
            Action::MergeResults(directory) => merge_results(&selection, directory),
        })
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("E2E runner error: {error}");
            ExitCode::FAILURE
        }
    }
}
