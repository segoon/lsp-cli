#![warn(clippy::pedantic)]

mod cli;
mod commands;
mod config;
mod detect;
mod env_vars;
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
use system_log::{log_unexpected_error, warn_if_log_file_is_large};

fn main() {
    warn_if_log_file_is_large();
    let cli_argv = env::args().skip(1).collect::<Vec<_>>();
    let raw_command = match parse_raw_args(cli_argv.clone()) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{message}");
            process::exit(2);
        }
    };

    let output = match raw_command {
        CliRawCommand::Commands(_) => run_commands(),
        CliRawCommand::Completion(completion_args) => run_completion(completion_args),
        CliRawCommand::Update(_) => {
            let cli = match update::load_cli_defaults_for_update() {
                Ok(cli) => cli,
                Err(error) => {
                    log_unexpected_error(&format!("failed to load lsp-cli defaults: {error}"));
                    eprintln!("failed to load lsp-cli defaults: {error}");
                    process::exit(1);
                }
            };
            let command = match resolve_command(raw_command, &cli) {
                Ok(command) => command,
                Err(error) => {
                    eprintln!("{error}");
                    process::exit(2);
                }
            };
            let config = config::ConfigStore {
                filetypes: Vec::new(),
                lsps: Vec::new(),
                cli,
            };
            run(command, &config)
        }
        raw_command => {
            if let Err(error) = update::ensure_data_available() {
                log_unexpected_error(&format!(
                    "failed to install lsp-cli data automatically: {error}"
                ));
                eprintln!("failed to install lsp-cli data automatically: {error}");
                process::exit(1);
            }
            let config_root = match default_config_root() {
                Ok(path) => path,
                Err(error) => {
                    log_unexpected_error(&format!("failed to resolve config root: {error}"));
                    eprintln!("failed to resolve config root: {error}");
                    process::exit(1);
                }
            };

            let mut config = match load_config_store(&config_root) {
                Ok(config) => config,
                Err(error) => {
                    log_unexpected_error(&format!(
                        "failed to load config from {}: {error}",
                        config_root.display()
                    ));
                    eprintln!(
                        "failed to load config from {}: {error}",
                        config_root.display()
                    );
                    process::exit(1);
                }
            };

            let cli_roots = config::CliConfigRoots::default();
            config.cli = match load_cli_config(&cli_roots.global, cli_roots.user.as_deref()) {
                Ok(cli) => cli,
                Err(error) => {
                    log_unexpected_error(&format!("failed to load lsp-cli defaults: {error}"));
                    eprintln!("failed to load lsp-cli defaults: {error}");
                    process::exit(1);
                }
            };

            let command = match resolve_command(raw_command, &config.cli) {
                Ok(command) => command,
                Err(error) => {
                    eprintln!("{error}");
                    process::exit(2);
                }
            };

            run(command, &config)
        }
    };

    match output {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(error) => {
            log_unexpected_error(&error);
            eprintln!("{error}");
            process::exit(1);
        }
    }
}
