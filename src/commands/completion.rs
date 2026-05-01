use crate::cli::{CompletionArgs, clap_command};
use clap_complete::generate;
use std::env;
use std::ffi::OsStr;
use std::io::Cursor;
use std::path::Path;

pub(super) fn run(args: CompletionArgs) -> Result<String, String> {
    let shell = args.shell.map_or_else(detect_current_shell, Ok)?;
    let mut command = clap_command();
    let mut output = Cursor::new(Vec::new());
    generate(shell, &mut command, "lsp-cli", &mut output);

    String::from_utf8(output.into_inner())
        .map_err(|error| format!("completion output was not valid UTF-8: {error}"))
}

pub(super) fn detect_current_shell() -> Result<clap_complete::Shell, String> {
    clap_complete::Shell::from_env()
        .ok_or(())
        .or_else(|()| detect_shell_from_env(env::var_os("SHELL").as_deref()))
}

pub(super) fn detect_shell_from_env(shell: Option<&OsStr>) -> Result<clap_complete::Shell, String> {
    let shell = shell.ok_or_else(|| {
        "could not detect current shell from $SHELL; pass one explicitly like `lsp-cli completion bash`"
            .to_string()
    })?;
    clap_complete::Shell::from_shell_path(shell).ok_or_else(|| {
        format!(
            "could not map current shell from $SHELL={}; pass one explicitly like `lsp-cli completion bash`",
            Path::new(shell).display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{detect_shell_from_env, run};
    use crate::cli::CompletionArgs;
    use clap_complete::Shell;

    #[test]
    fn generates_bash_completion_script() {
        let output = run(CompletionArgs {
            shell: Some(Shell::Bash),
        })
        .expect("completion script should generate");

        assert!(output.contains("lsp-cli"));
        assert!(output.contains("detect"));
        assert!(output.contains("grep"));
        assert!(output.contains("references"));
        assert!(output.contains("callers"));
        assert!(output.contains("callees"));
        assert!(output.contains("definition"));
        assert!(output.contains("declaration"));
        assert!(output.contains("list-files"));
        assert!(output.contains("list-functions"));
        assert!(output.contains("list-symbols"));
        assert!(output.contains("completion"));
    }

    #[test]
    fn detects_shell_from_shell_path() {
        assert_eq!(
            detect_shell_from_env(Some("/bin/zsh".as_ref())),
            Ok(Shell::Zsh)
        );
        assert_eq!(
            detect_shell_from_env(Some("/usr/bin/powershell".as_ref())),
            Ok(Shell::PowerShell)
        );
    }

    #[test]
    fn errors_when_shell_env_is_missing() {
        assert_eq!(
            detect_shell_from_env(None),
            Err(
                "could not detect current shell from $SHELL; pass one explicitly like `lsp-cli completion bash`"
                    .to_string()
            )
        );
    }

    #[test]
    fn errors_when_shell_env_is_unsupported() {
        assert_eq!(
            detect_shell_from_env(Some("/bin/sh".as_ref())),
            Err(
                "could not map current shell from $SHELL=/bin/sh; pass one explicitly like `lsp-cli completion bash`"
                    .to_string()
            )
        );
    }
}
