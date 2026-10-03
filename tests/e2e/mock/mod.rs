use std::path::Path;

mod catalog;
mod filesystem;
mod fixture;
mod lifecycle;
mod local_fixture;
mod queries;
mod update;

pub use catalog::catalog_command_paths_are_covered;
pub use filesystem::filesystem_command_paths_are_covered;
pub use lifecycle::{lifecycle_command_paths_are_covered, run_forwards_a_complete_lsp_exchange};
pub use queries::{
    lsp_fixture_command_paths_are_covered, unadvertised_capabilities_produce_user_facing_errors,
};
pub use update::update_command_path_uses_local_release_fixture;

/// Run a mock case with the executable supplied by the owning package's integration test.
pub fn run(binary: &Path, case: fn()) {
    crate::initialize(binary);
    case();
}
