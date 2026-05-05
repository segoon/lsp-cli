use crate::cli::{CommandsArgs, clap_command};
use crate::error::Result;

#[allow(clippy::unnecessary_wraps)]
pub(super) fn run(_args: &CommandsArgs) -> Result<String> {
    Ok(render_commands())
}

pub(crate) fn render_commands() -> String {
    clap_command()
        .get_subcommands()
        .filter(|command| command.get_name() != "help")
        .map(clap::Command::get_name)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{render_commands, run};
    use crate::cli::CommandsArgs;

    #[test]
    fn renders_top_level_commands_in_cli_order() {
        assert_eq!(
            render_commands(),
            concat!(
                "commands\n",
                "daemon\n",
                "stop\n",
                "stop-all\n",
                "languages\n",
                "servers\n",
                "server-capabilities\n",
                "detect\n",
                "diagnostics\n",
                "format\n",
                "grep\n",
                "list-symbols\n",
                "list-functions\n",
                "list-files\n",
                "references\n",
                "callers\n",
                "callees\n",
                "definition\n",
                "declaration\n",
                "build-index\n",
                "update\n",
                "completion\n",
                "agent-skill\n",
                "run"
            )
        );
    }

    #[test]
    fn excludes_clap_help_subcommand() {
        assert!(!render_commands().lines().any(|command| command == "help"));
    }

    #[test]
    fn run_returns_rendered_commands() {
        assert_eq!(
            run(&CommandsArgs).expect("commands should render"),
            render_commands()
        );
    }
}
