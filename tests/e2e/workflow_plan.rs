use std::collections::BTreeMap;
use std::path::Path;

use crate::manifest::Manifest;
pub use crate::manifest::coverage_cases::{
    DownloadableServer, InstallationFamily, RegistrySnapshot, WorkflowPackage, WorkflowPlan,
    WorkflowSelector,
};

/// The validated suite inventory used by the CI workflow planner.
pub struct WorkflowManifest(Manifest);

impl WorkflowManifest {
    pub fn load_validated(repository: &Path) -> Result<Self, String> {
        Manifest::load_validated(repository).map(Self)
    }

    pub fn downloadable_servers(&self) -> Result<Vec<DownloadableServer>, String> {
        self.0.downloadable_servers()
    }

    pub fn workflow_plan(
        &self,
        selector: WorkflowSelector,
        value: Option<&str>,
        packages: &BTreeMap<String, WorkflowPackage>,
        snapshot: &RegistrySnapshot,
    ) -> Result<(WorkflowPlan, String), String> {
        self.0.workflow_plan(selector, value, packages, snapshot)
    }
}
