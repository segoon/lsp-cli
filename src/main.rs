#![warn(clippy::pedantic)]

mod cli;
mod commands;
mod config;
mod detect;
mod lsp;
mod mason;
mod runtime_state;
mod suggest;

#[cfg(test)]
mod test_support;

use std::env;
use std::process;

use cli::{Command as CliCommand, parse_args};
use commands::{run, run_completion};
use config::{default_config_root, load_config_store};

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            process::exit(2);
        }
    };

    let output = match args {
        CliCommand::Completion(args) => run_completion(args),
        args => {
            let config_root = match default_config_root() {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("failed to resolve config root: {error}");
                    process::exit(1);
                }
            };

            let config = match load_config_store(&config_root) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!(
                        "failed to load config from {}: {error}",
                        config_root.display()
                    );
                    process::exit(1);
                }
            };

            run(args, &config)
        }
    };

    match output {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}
