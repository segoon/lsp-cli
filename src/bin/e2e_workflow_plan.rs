#![expect(
    dead_code,
    reason = "The planner reuses manifest and Mason modules with broader production and test APIs."
)]
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::string_slice,
    reason = "Reused production and E2E modules are linted in their primary crate targets."
)]

use std::collections::BTreeMap;
use std::path::Path;

#[path = "../../tests/e2e/case_files.rs"]
mod case_files;
#[path = "../env_vars.rs"]
mod env_vars;
#[path = "../error.rs"]
mod error;
#[path = "../fs.rs"]
mod fs;
#[path = "../hash.rs"]
mod hash;
#[path = "../../tests/e2e/manifest.rs"]
mod manifest;
#[path = "../../tests/e2e/manifest_data.rs"]
mod manifest_data;
#[path = "e2e_workflow_plan/mason.rs"]
mod mason;
#[path = "../runtime_state.rs"]
mod runtime_state;
#[cfg(test)]
#[path = "e2e_workflow_plan/test_support.rs"]
mod test_support;

use manifest::Manifest;
use manifest::coverage_cases::{
    InstallationFamily, RegistrySnapshot, WorkflowPackage, WorkflowSelector,
};
use mason::registry::MasonRegistry;
use runtime_state::RuntimeState;

const REGISTRY_SNAPSHOT_OUTPUT_ENV: &str = "E2E_REGISTRY_SNAPSHOT_OUTPUT";
const REGISTRY_SNAPSHOT_FILES: [&str; 2] = ["registry.json", "metadata.json"];

fn main() -> Result<(), String> {
    let selector = parse_selector(&required_environment("E2E_SELECTOR")?)?;
    let value = std::env::var("E2E_SELECTOR_VALUE").ok();
    let manifest = Manifest::load_validated(repository_root())?;
    let cache = tempfile::tempdir()
        .map_err(|error| format!("failed to create temporary Mason registry cache: {error}"))?;
    let registry_state = RuntimeState::new(cache.path().join("mason"));
    let registry = MasonRegistry::load(&registry_state)
        .map_err(|error| format!("failed to load current Mason registry: {error}"))?;
    let mut packages = BTreeMap::new();
    for server in manifest.downloadable_servers()? {
        let package = registry
            .package_for_detected(&server.id, &server.name, &server.program)
            .ok_or_else(|| format!("Mason registry has no package for {:?}", server.id))?;
        let family = InstallationFamily::from_source_id(&package.source.id)
            .map_err(|error| format!("server {:?}: {error}", server.id))?;
        packages.insert(
            server.id,
            WorkflowPackage {
                installation_family: family,
                source_id: package.source.id.clone(),
            },
        );
    }
    let snapshot = read_registry_snapshot(&registry_state)?;
    let (plan, report) =
        manifest.workflow_plan(selector, value.as_deref(), &packages, &snapshot)?;
    std::fs::write(
        required_environment("E2E_PLAN_OUTPUT")?,
        serde_json::to_vec(&plan).map_err(|error| format!("failed to serialize plan: {error}"))?,
    )
    .map_err(|error| format!("failed to write workflow plan: {error}"))?;
    std::fs::write(required_environment("E2E_REPORT_OUTPUT")?, report)
        .map_err(|error| format!("failed to write workflow report: {error}"))?;
    if let Ok(output) = std::env::var(REGISTRY_SNAPSHOT_OUTPUT_ENV) {
        persist_registry_snapshot(&registry_state, Path::new(&output))?;
    }
    // Explicit close prevents registry downloads from accumulating across workflow planning runs.
    cache
        .close()
        .map_err(|error| format!("failed to remove Mason registry cache: {error}"))
}

fn read_registry_snapshot(state: &RuntimeState) -> Result<RegistrySnapshot, String> {
    let path = state.registry_metadata_path();
    let contents = std::fs::read(&path).map_err(|error| {
        format!(
            "failed to read Mason registry metadata {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_slice(&contents).map_err(|error| {
        format!(
            "failed to parse Mason registry metadata {}: {error}",
            path.display()
        )
    })
}

fn persist_registry_snapshot(state: &RuntimeState, output: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output).map_err(|error| {
        format!(
            "failed to create Mason registry snapshot directory {}: {error}",
            output.display()
        )
    })?;
    for name in REGISTRY_SNAPSHOT_FILES {
        let source = state.registry_dir().join(name);
        let destination = output.join(name);
        std::fs::copy(&source, &destination).map_err(|error| {
            format!(
                "failed to copy Mason registry snapshot {} to {}: {error}",
                source.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn required_environment(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_error| format!("{name} must be set"))
}

fn parse_selector(value: &str) -> Result<WorkflowSelector, String> {
    match value {
        "all" => Ok(WorkflowSelector::All),
        "language" => Ok(WorkflowSelector::Language),
        "server" => Ok(WorkflowSelector::Server),
        "installation-family" => Ok(WorkflowSelector::InstallationFamily),
        _ => Err(format!("unknown E2E selector {value:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_the_registry_files_used_by_the_plan() {
        let source = tempfile::tempdir().expect("source directory should initialize");
        let output = tempfile::tempdir().expect("output directory should initialize");
        let state = RuntimeState::new(source.path().join("state"));
        std::fs::create_dir_all(state.registry_dir())
            .expect("registry directory should be created");
        for (name, contents) in [
            ("registry.json", b"registry".as_slice()),
            ("metadata.json", b"metadata".as_slice()),
        ] {
            std::fs::write(state.registry_dir().join(name), contents)
                .expect("source file should be written");
        }

        persist_registry_snapshot(&state, output.path()).expect("snapshot should persist");

        for name in REGISTRY_SNAPSHOT_FILES {
            assert_eq!(
                std::fs::read(output.path().join(name)).expect("snapshot should be readable"),
                std::fs::read(state.registry_dir().join(name)).expect("source should be readable")
            );
        }
    }
}
