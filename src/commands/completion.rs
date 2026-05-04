use crate::cli::{CompletionArgs, clap_command};
use crate::config::{default_config_root, load_config_store};
use clap::builder::PossibleValuesParser;
use clap_complete::{Shell, generate};
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::io::Cursor;
use std::path::Path;

pub(super) fn run(args: CompletionArgs) -> Result<String, String> {
    let shell = args.shell.map_or_else(detect_current_shell, Ok)?;
    let mut command = completion_command()?;
    let mut output = Cursor::new(Vec::new());
    generate(shell, &mut command, "lsp-cli", &mut output);

    String::from_utf8(output.into_inner())
        .map(|output| normalize_completion_output(shell, output, "lsp-cli"))
        .map_err(|error| format!("completion output was not valid UTF-8: {error}"))
}

fn normalize_completion_output(shell: Shell, output: String, bin_name: &str) -> String {
    if shell != Shell::Bash || !bin_name.contains('-') {
        return output;
    }

    // clap_complete 4.6.3 emits bash handler labels using `__subcmd__` for
    // hyphens in the root binary name while the dispatcher uses `__`, which
    // makes subcommand-specific completion unreachable for binaries like
    // `lsp-cli`. Normalize the generated labels to the dispatcher form.
    output.replace(
        &bin_name.replace('-', "__subcmd__"),
        &bin_name.replace('-', "__"),
    )
}

fn completion_command() -> Result<clap::Command, String> {
    let values = load_completion_values()?;
    Ok(attach_completion_values(clap_command(), &values))
}

fn load_completion_values() -> Result<CompletionValues, String> {
    let config_root = default_config_root()
        .map_err(|error| format!("failed to resolve config root for completion: {error}"))?;
    let config = load_config_store(&config_root).map_err(|error| {
        format!(
            "failed to load completion values from {}: {error}",
            config_root.display()
        )
    })?;

    Ok(CompletionValues {
        languages: config
            .filetypes
            .into_iter()
            .map(|filetype| filetype.id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        lsps: config
            .lsps
            .into_iter()
            .map(|lsp| lsp.name)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    })
}

fn attach_completion_values(command: clap::Command, values: &CompletionValues) -> clap::Command {
    command
        .mut_args(|arg| match arg.get_long() {
            Some("lang") => arg.value_parser(PossibleValuesParser::new(values.languages.clone())),
            Some("lsp") => arg.value_parser(PossibleValuesParser::new(values.lsps.clone())),
            _ => arg,
        })
        .mut_subcommands(|subcommand| attach_completion_values(subcommand, values))
}

struct CompletionValues {
    languages: Vec<String>,
    lsps: Vec<String>,
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
    use super::{detect_shell_from_env, normalize_completion_output, run};
    use crate::cli::CompletionArgs;
    use crate::test_support::{TestDir, env_var, with_env_vars};
    use clap_complete::Shell;
    use std::process::Command;

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
        assert!(output.contains("agent-skill"));
    }

    #[test]
    fn normalizes_bash_labels_for_hyphenated_binary_names() {
        assert_eq!(
            normalize_completion_output(
                Shell::Bash,
                "lsp__subcmd__cli__subcmd__detect".to_string(),
                "lsp-cli",
            ),
            "lsp__cli__subcmd__detect"
        );
    }

    #[test]
    fn bash_detect_lsp_completion_reaches_value_candidates() {
        let script = run(CompletionArgs {
            shell: Some(Shell::Bash),
        })
        .expect("bash completion should generate");

        let output = Command::new("/bin/bash")
            .arg("-lc")
            .arg(
                "source /dev/stdin && COMP_WORDS=(lsp-cli detect playground/c --lsp \"\") && COMP_CWORD=4 && COMPREPLY=() && _lsp-cli lsp-cli \"\" --lsp && printf '%s\n' \"${COMPREPLY[@]}\"",
            )
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write as _;

                child
                    .stdin
                    .as_mut()
                    .expect("stdin should be piped")
                    .write_all(script.as_bytes())?;
                child.wait_with_output()
            })
            .expect("bash subprocess should run");

        let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
        assert!(stdout.contains("clangd"));
    }

    #[test]
    fn includes_configured_languages_and_servers_in_completion_script() {
        let config = TestDir::new("completion-config");
        config.write_file(
            "filetypes/python.yaml",
            "extensions:\n  - py\npatterns: []\n",
        );
        config.write_file("filetypes/rust.yaml", "extensions:\n  - rs\npatterns: []\n");
        config.write_file(
            "lsp/pyright.yaml",
            "filetypes:\n  - python\nroot_markers: []\nname: pyright\ncmdline: pyright-langserver --stdio\n",
        );
        config.write_file(
            "lsp/rust_analyzer.yaml",
            "filetypes:\n  - rust\nroot_markers: []\nname: rust-analyzer\ncmdline: rust-analyzer\n",
        );

        let output = with_env_vars(&[env_var("LSP_DATA", config.path())], || {
            run(CompletionArgs {
                shell: Some(Shell::Bash),
            })
        })
        .expect("completion script should include configured values");

        assert!(output.contains("python"));
        assert!(output.contains("rust"));
        assert!(output.contains("pyright"));
        assert!(output.contains("rust-analyzer"));
    }

    #[test]
    fn errors_when_completion_values_cannot_be_loaded() {
        let config = TestDir::new("completion-missing-config");

        let error = with_env_vars(&[env_var("LSP_DATA", config.path())], || {
            run(CompletionArgs {
                shell: Some(Shell::Bash),
            })
        })
        .expect_err("completion should fail when config cannot be loaded");

        assert!(error.contains("failed to load completion values from"));
        assert!(error.contains("missing directory"));
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
