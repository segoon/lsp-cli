use crate::cli::UpdateArgs;
use crate::config::ConfigStore;
use crate::error::Result;

pub(super) fn run(_: &UpdateArgs, config: &ConfigStore) -> Result<String> {
    crate::update::run_update_with_cli(&config.cli)
}
