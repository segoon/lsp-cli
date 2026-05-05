use crate::cli::RunArgs;
use crate::commands::common::prepare_workspace;
use crate::config::ConfigStore;
use crate::error::{Error, Result};
use crate::system_log::{log_lsp_server_cmdline, log_lsp_server_cwd, log_lsp_server_starting};
use std::process::{Command, Stdio};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub(super) fn run(args: &RunArgs, config: &ConfigStore) -> Result<String> {
    let workspace = prepare_workspace(
        &args.path,
        args.server.server(),
        args.server.language(),
        args.server.download,
        config,
    )?;
    let server = workspace.server;
    let Some(program) = server.command.first() else {
        return Err(Error::unexpected(format!(
            "selected LSP server {} has an empty command",
            server.server
        )));
    };

    if args.server.debug {
        eprintln!("LSP server: {}", server.command.join(" "));
    }

    log_lsp_server_starting();
    log_lsp_server_cmdline(&server.command);
    log_lsp_server_cwd(&server.workspace_root);

    let mut command = Command::new(program);
    command
        .args(&server.command[1..])
        .current_dir(&server.workspace_root)
        .stderr(if args.server.debug {
            Stdio::inherit()
        } else {
            Stdio::null()
        });

    #[cfg(unix)]
    {
        Err(format_exec_error(program, &command.exec()))
    }

    #[cfg(not(unix))]
    {
        let _ = command;
        Err(Error::unexpected(
            "lsp-cli run is only supported on unix-like systems",
        ))
    }
}

fn format_exec_error(program: &str, error: &std::io::Error) -> Error {
    match error.kind() {
        std::io::ErrorKind::NotFound if !program.contains(std::path::MAIN_SEPARATOR) => {
            Error::missing_executable(format!(
                "LSP server executable `{program}` is not installed or not in $PATH"
            ))
        }
        std::io::ErrorKind::NotFound => Error::missing_executable(format!(
            "configured LSP server executable `{program}` was not found"
        )),
        _ => Error::unexpected(format!("failed to execute LSP server `{program}`: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::format_exec_error;
    use crate::error::Error;

    #[test]
    fn formats_missing_binary_error() {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");

        assert!(matches!(
            format_exec_error("ast-grep", &error),
            Error::MissingExecutable(message)
                if message == "LSP server executable `ast-grep` is not installed or not in $PATH"
        ));
    }
}
