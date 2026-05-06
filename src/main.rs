#![warn(clippy::pedantic)]
#![expect(
    clippy::allow_attributes,
    reason = "Existing targeted suppressions predate workspace lint activation and will be migrated incrementally."
)]
#![expect(
    clippy::allow_attributes_without_reason,
    reason = "Existing targeted suppressions predate workspace lint activation and will be migrated incrementally."
)]
#![expect(
    clippy::indexing_slicing,
    reason = "Pre-existing indexing-heavy code needs a dedicated refactor to preserve behavior and readability."
)]
#![expect(
    clippy::string_slice,
    reason = "Pre-existing UTF-8-sensitive slicing code needs targeted review before rewriting."
)]
#![expect(
    clippy::map_err_ignore,
    reason = "Several user-facing parse errors intentionally replace low-level details and need a broader error-design pass."
)]
#![expect(
    clippy::let_underscore_must_use,
    reason = "Best-effort cleanup and notification paths intentionally ignore some must-use results until those flows are refactored."
)]
#![expect(
    clippy::undocumented_unsafe_blocks,
    reason = "Pre-existing unsafe call sites need targeted safety comments and review separate from this lint-activation change."
)]
#![expect(
    clippy::panic,
    reason = "A few invariant checks still panic today and need a separate API design pass."
)]
#![expect(
    clippy::unreachable,
    reason = "Command dispatch still uses one invariant unreachable branch that should be revisited separately."
)]

mod cli;
mod commands;
mod config;
mod detect;
mod env_vars;
mod error;
mod fs;
mod hash;
mod lsp;
mod mason;
mod runtime_state;
mod server_stderr;
mod suggest;
mod system_log;
mod update;

#[cfg(test)]
mod test_support;

use std::env;
use std::process;

use cli::{RawCommand as CliRawCommand, parse_raw_args, resolve_command};
use commands::{run, run_commands, run_completion};
use config::{default_config_root, load_cli_config, load_config_store};
use error::{Error, Result};
use system_log::{log_unexpected_error, warn_if_log_file_is_large};

fn main() {
    warn_if_log_file_is_large();
    let raw_command = parse_command_or_exit();

    let output = match raw_command {
        CliRawCommand::Commands(_) => run_commands(),
        CliRawCommand::Completion(completion_args) => run_completion(completion_args),
        CliRawCommand::Update(_) => run_update_command(raw_command),
        raw_command => run_with_loaded_config(raw_command),
    };

    match output {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(error) => {
            if error.should_log_as_unexpected() {
                log_unexpected_error(&error.to_string());
            }
            eprintln!("{error}");
            process::exit(error.exit_code());
        }
    }
}

fn parse_command_or_exit() -> CliRawCommand {
    let cli_argv = env::args().skip(1).collect::<Vec<_>>();
    parse_raw_args(cli_argv).unwrap_or_else(|error| exit_with_error(&error))
}

fn run_update_command(raw_command: CliRawCommand) -> Result<String> {
    let cli = update::load_cli_defaults_for_update()?;
    let command = resolve_command(raw_command, &cli)?;
    let config = config::ConfigStore {
        filetypes: Vec::new(),
        lsps: Vec::new(),
        cli,
    };
    run(command, &config)
}

fn run_with_loaded_config(raw_command: CliRawCommand) -> Result<String> {
    if let Err(error) = update::ensure_data_available() {
        return Err(Error::unexpected(format!(
            "failed to install lsp-cli data automatically: {error}"
        )));
    }

    let config_root = default_config_root()?;
    let mut config = load_config_store(&config_root).map_err(|error| {
        error.with_prefix(format!(
            "failed to load config from {}",
            config_root.display()
        ))
    })?;
    let cli_roots = config::CliConfigRoots::default();
    config.cli = load_cli_config(&cli_roots.global, cli_roots.user.as_deref())
        .map_err(|error| error.with_prefix("failed to load lsp-cli defaults"))?;
    let command = resolve_command(raw_command, &config.cli)?;

    run(command, &config)
}

fn exit_with_error(error: &Error) -> ! {
    if error.should_log_as_unexpected() {
        log_unexpected_error(&error.to_string());
    }
    eprintln!("{error}");
    process::exit(error.exit_code())
}
