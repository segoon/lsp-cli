use crate::cli::{AgentSkillArgs, clap_command};
use clap::Arg;
use clap::Command as ClapCommand;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

const TEMPLATE: &str = include_str!("agent_skill.template.md");
const IGNORED_COMMANDS: &[&str] = &[
    "commands",
    "daemon",
    "stop",
    "stop-all",
    "server-capabilities",
    "detect",
    "languages",
    "build-index",
    "update",
    "completion",
    "agent-skill",
    "run",
];
const IGNORED_OPTIONS: &[&str] = &[
    "debug",
    "no-debug",
    "timeout",
    "download",
    "no-download",
    "no-detach",
    "detach",
    "no-json",
    "wait-for-index",
];

#[allow(clippy::unnecessary_wraps)]
pub(super) fn run(_args: &AgentSkillArgs) -> Result<String, String> {
    Ok(render_skill())
}

fn render_skill() -> String {
    let root = clap_command();
    let mut replacements = command_replacements(&root);
    replacements.extend(option_replacements(&root));
    render_template(TEMPLATE, &replacements)
}

fn command_replacements(root: &ClapCommand) -> BTreeMap<String, String> {
    validate_ignored_commands(root);

    root.get_subcommands()
        .filter(|command| command.get_name() != "help")
        .filter(|command| !IGNORED_COMMANDS.contains(&command.get_name()))
        .map(|command| {
            let placeholder = command_placeholder(command.get_name()).unwrap_or_else(|| {
                panic!(
                    "non-ignored command `{}` is missing an agent-skill template placeholder",
                    command.get_name()
                )
            });
            (placeholder.to_string(), command_about(command))
        })
        .collect()
}

fn option_replacements(root: &ClapCommand) -> BTreeMap<String, String> {
    let mut counts = BTreeMap::<String, BTreeMap<String, usize>>::new();
    let mut known_options = BTreeSet::<String>::new();

    for command in root
        .get_subcommands()
        .filter(|command| command.get_name() != "help")
        .filter(|command| !IGNORED_COMMANDS.contains(&command.get_name()))
    {
        for arg in command.get_arguments() {
            let Some(long) = arg.get_long() else {
                continue;
            };
            if long == "help" || arg.is_hide_set() {
                continue;
            }

            known_options.insert(long.to_string());
            if IGNORED_OPTIONS.contains(&long) {
                continue;
            }

            let help = option_help(arg);
            *counts
                .entry(long.to_string())
                .or_default()
                .entry(help)
                .or_default() += 1;
        }
    }

    for ignored in IGNORED_OPTIONS {
        assert!(
            known_options.contains(*ignored),
            "ignored agent-skill option `--{ignored}` does not exist"
        );
    }

    counts
        .into_iter()
        .map(|(long, variants)| {
            let help = select_option_help(&long, &variants);
            (option_placeholder(&long), help)
        })
        .collect()
}

fn validate_ignored_commands(root: &ClapCommand) {
    let known = root
        .get_subcommands()
        .map(ClapCommand::get_name)
        .collect::<BTreeSet<_>>();

    for ignored in IGNORED_COMMANDS {
        assert!(
            known.contains(ignored),
            "ignored agent-skill command `{ignored}` does not exist"
        );
    }
}

fn command_placeholder(command: &str) -> Option<&'static str> {
    match command {
        "grep" => Some("CMD/GREP"),
        "list-symbols" => Some("CMD/LIST_SYMBOLS"),
        "list-functions" => Some("CMD/LIST_FUNCTIONS"),
        "list-files" => Some("CMD/LIST_FILES"),
        "definition" => Some("CMD/DEFINITION"),
        "declaration" => Some("CMD/DECLARATION"),
        "references" => Some("CMD/REFERENCES"),
        "callers" => Some("CMD/CALLERS"),
        "callees" => Some("CMD/CALLEES"),
        "diagnostics" => Some("CMD/DIAGNOSTICS"),
        "format" => Some("CMD/FORMAT"),
        "languages" => Some("CMD/LANGUAGES"),
        "servers" => Some("CMD/SERVERS"),
        _ => None,
    }
}

fn option_placeholder(long: &str) -> String {
    format!("OPT/{}", long.replace('-', "_").to_uppercase())
}

fn select_option_help(long: &str, variants: &BTreeMap<String, usize>) -> String {
    let max_count = variants
        .values()
        .copied()
        .max()
        .unwrap_or_else(|| panic!("agent-skill option `--{long}` has no help variants"));
    let best = variants
        .iter()
        .filter(|(_, count)| **count == max_count)
        .map(|(help, _)| help.clone())
        .collect::<Vec<_>>();

    assert!(
        best.len() == 1,
        "agent-skill option `--{long}` has ambiguous help variants: {}",
        best.join(" | ")
    );
    best[0].clone()
}

fn render_template(template: &str, replacements: &BTreeMap<String, String>) -> String {
    let placeholder_regex =
        Regex::new(r"\{([A-Z0-9/_]+)\}").expect("placeholder regex should compile");
    let used = placeholder_regex
        .captures_iter(template)
        .map(|captures| captures[1].to_string())
        .collect::<BTreeSet<_>>();

    for placeholder in &used {
        assert!(
            replacements.contains_key(placeholder.as_str()),
            "agent-skill template contains unknown placeholder `{{{placeholder}}}`"
        );
    }

    for placeholder in replacements.keys() {
        assert!(
            used.contains(placeholder),
            "agent-skill template argument `{{{placeholder}}}` was provided but never used"
        );
    }

    placeholder_regex
        .replace_all(template, |captures: &regex::Captures<'_>| {
            replacements
                .get(&captures[1].to_string())
                .unwrap_or_else(|| panic!("missing replacement for `{{{}}}`", &captures[1]))
                .clone()
        })
        .into_owned()
}

fn command_about(command: &ClapCommand) -> String {
    command.get_about().map_or_else(
        || "No summary available.".to_string(),
        clap::builder::StyledStr::to_string,
    )
}

fn option_help(arg: &Arg) -> String {
    arg.get_help().map_or_else(
        || "No help available.".to_string(),
        clap::builder::StyledStr::to_string,
    )
}

#[cfg(test)]
mod tests {
    use super::{render_skill, render_template, run};
    use crate::cli::AgentSkillArgs;
    use std::collections::BTreeMap;

    #[test]
    fn renders_curated_skill_sections() {
        let markdown = render_skill();

        assert!(markdown.starts_with("---\nname: lsp-cli\n"));
        assert!(markdown.contains("# lsp-cli skill"));
        assert!(markdown.contains("## Core commands"));
        assert!(markdown.contains("## Setup and troubleshooting"));
        assert!(
            markdown
                .contains("Purpose: Search workspace symbols (regex syntax is server-dependent)")
        );
        assert!(markdown.contains("Purpose: Find definitions of a symbol name"));
        assert!(markdown.contains("Purpose: List known languages"));
        assert!(markdown.contains("Purpose: List known LSP servers"));
        assert!(markdown.contains("Prefer `--json`"));
    }

    #[test]
    fn excludes_plumbing_commands_from_generated_skill() {
        let markdown = render_skill();

        assert!(!markdown.contains("### `completion`"));
        assert!(!markdown.contains("### `run`"));
        assert!(!markdown.contains("### `commands`"));
        assert!(!markdown.contains("### `stop`"));
        assert!(!markdown.contains("### `stop-all`"));
        assert!(!markdown.contains("### `update`"));
    }

    #[test]
    fn run_returns_markdown_for_stdout() {
        let markdown = run(&AgentSkillArgs {}).expect("skill should render");

        assert!(markdown.starts_with("---\n"));
        assert!(markdown.contains("# lsp-cli skill"));
    }

    #[test]
    fn output_starts_with_frontmatter_block() {
        let markdown = run(&AgentSkillArgs {}).expect("skill should render to stdout");

        assert!(markdown.starts_with("---\nname: lsp-cli\n"));
        assert!(markdown.contains("\n---\n\n# lsp-cli skill"));
    }

    #[test]
    fn agent_skill_does_not_panic() {
        let result = std::panic::catch_unwind(render_skill);

        assert!(result.is_ok(), "agent-skill should not panic");
    }

    #[test]
    fn panics_for_unknown_template_placeholders() {
        let args = BTreeMap::from([("CMD/GREP".to_string(), "grep about".to_string())]);
        let result = std::panic::catch_unwind(|| render_template("{CMD/GREP} {CMD/NOPE}", &args));

        assert!(result.is_err(), "unknown placeholders should panic");
    }

    #[test]
    fn panics_for_unused_template_arguments() {
        let args = BTreeMap::from([
            ("CMD/GREP".to_string(), "grep about".to_string()),
            ("CMD/FORMAT".to_string(), "format about".to_string()),
        ]);
        let result = std::panic::catch_unwind(|| render_template("{CMD/GREP}", &args));

        assert!(result.is_err(), "unused template arguments should panic");
    }

    #[test]
    fn renders_option_placeholders_from_clap_help() {
        let markdown = render_skill();

        assert!(markdown.contains("`--json`: Print results as JSON."));
        assert!(markdown.contains(
            "`--limit <N>`: Maximum number of results to print. Mainly usable for code agents."
        ));
        assert!(markdown.contains("`--lsp <LSP>`: Use a specific configured LSP server."));
        assert!(markdown.contains("`--lang <LANG>`: Select this language."));
        assert!(
            markdown.contains(
                "`-l, --files-with-matches`: Print only file paths that contain matches."
            )
        );
    }
}
