use crate::cli::RunArgs;
use crate::commands::common::prepare_workspace;
use crate::config::ConfigStore;
use std::process::{Command, Stdio};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub(super) fn run(args: &RunArgs, config: &ConfigStore) -> Result<String, String> {
    let workspace = prepare_workspace(
        &args.path,
        args.lsp.as_deref(),
        args.lang.as_deref(),
        args.download,
        config,
    )?;
    let server = workspace.server;
    let Some(program) = server.command.first() else {
        return Err(format!(
            "selected LSP server {} has an empty command",
            server.server
        ));
    };

    if args.debug {
        eprintln!("LSP server: {}", server.command.join(" "));
    }

    let mut command = Command::new(program);
    command
        .args(&server.command[1..])
        .current_dir(&server.workspace_root)
        .stderr(if args.debug {
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
        Err("lsp-cli run is only supported on unix-like systems".to_string())
    }
}

fn format_exec_error(program: &str, error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound if !program.contains(std::path::MAIN_SEPARATOR) => {
            format!("LSP server executable `{program}` is not installed or not in $PATH")
        }
        std::io::ErrorKind::NotFound => {
            format!("configured LSP server executable `{program}` was not found")
        }
        _ => format!("failed to execute LSP server `{program}`: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::format_exec_error;

    #[test]
    fn formats_missing_binary_error() {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");

        assert_eq!(
            format_exec_error("ast-grep", &error),
            "LSP server executable `ast-grep` is not installed or not in $PATH"
        );
    }
}
