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
use manifest::coverage_cases::{InstallationFamily, WorkflowSelector};
use mason::registry::MasonRegistry;
use runtime_state::RuntimeState;

fn main() -> Result<(), String> {
    let selector = parse_selector(&required_environment("E2E_SELECTOR")?)?;
    let value = std::env::var("E2E_SELECTOR_VALUE").ok();
    let manifest = Manifest::load_validated(repository_root())?;
    let cache = tempfile::tempdir()
        .map_err(|error| format!("failed to create temporary Mason registry cache: {error}"))?;
    let registry = MasonRegistry::load(&RuntimeState::new(cache.path().join("mason")))
        .map_err(|error| format!("failed to load current Mason registry: {error}"))?;
    let mut families = BTreeMap::new();
    for server in manifest.downloadable_servers()? {
        let package = registry
            .package_for_detected(&server.id, &server.name, &server.program)
            .ok_or_else(|| format!("Mason registry has no package for {:?}", server.id))?;
        let family = InstallationFamily::from_source_id(&package.source.id)
            .map_err(|error| format!("server {:?}: {error}", server.id))?;
        families.insert(server.id, family);
    }
    let (plan, report) = manifest.workflow_plan(selector, value.as_deref(), &families)?;
    std::fs::write(
        required_environment("E2E_PLAN_OUTPUT")?,
        serde_json::to_vec(&plan).map_err(|error| format!("failed to serialize plan: {error}"))?,
    )
    .map_err(|error| format!("failed to write workflow plan: {error}"))?;
    std::fs::write(required_environment("E2E_REPORT_OUTPUT")?, report)
        .map_err(|error| format!("failed to write workflow report: {error}"))?;
    // Explicit close prevents registry downloads from accumulating across workflow planning runs.
    cache
        .close()
        .map_err(|error| format!("failed to remove Mason registry cache: {error}"))
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
