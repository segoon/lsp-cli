use crate::cli::UpdateArgs;
use crate::config::ConfigStore;

pub(super) fn run(_: &UpdateArgs, config: &ConfigStore) -> Result<String, String> {
    crate::update::run_update_with_cli(&config.cli)
}
