use crate::cli::{AgentSkillArgs, clap_command};
use crate::config::ConfigStore;
use clap::{Arg, Command as ClapCommand};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(super) fn run(args: &AgentSkillArgs, config: &ConfigStore) -> Result<String, String> {
    let skill = render_skill(config)?;
    if args.path == Path::new("-") {
        return Ok(skill);
    }

    if let Some(parent) = args.path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create parent directory {}: {error}",
                parent.display()
            )
        })?;
    }

    fs::write(&args.path, skill).map_err(|error| {
        format!(
            "failed to write generated skill file to {}: {error}",
            args.path.display()
        )
    })?;
    Ok(format!("wrote agent skill to {}", args.path.display()))
}

fn render_skill(config: &ConfigStore) -> Result<String, String> {
    let root = clap_command();
    let mut sections = vec![
        "# lsp-cli skill".to_string(),
        "This skill helps a code agent use `lsp-cli` for semantic code navigation, diagnostics, and formatting from the terminal without editor-specific LSP integration.".to_string(),
        render_when_to_use(),
        render_rules_of_thumb(),
        render_known_values(config),
        render_command_group("## Core commands", CORE_COMMANDS, &root)?,
        render_command_group("## Setup and troubleshooting", SUPPORT_COMMANDS, &root)?,
        render_limitations(),
    ];
    sections.retain(|section| !section.is_empty());
    Ok(sections.join("\n\n"))
}

fn render_when_to_use() -> String {
    [
        "## When to use lsp-cli",
        "- Use it when you need semantic workspace navigation instead of plain text search.",
        "- Use it when the agent runs in a shell, container, CI job, or SSH session without editor-managed LSP integration.",
        "- Use it when the repository uses a rare or proprietary language server that the agent does not know how to configure directly.",
        "- Use it when you need LSP diagnostics or formatting as part of an edit/verify loop.",
    ]
    .join("\n")
}

fn render_rules_of_thumb() -> String {
    [
        "## Rules of thumb",
        "- Prefer `--json` when the output will be parsed or summarized by the agent.",
        "- Prefer `--limit <N>` to avoid flooding the agent context with large workspaces.",
        "- Prefer `--detach` for repeated semantic queries so the server stays warm in a background daemon.",
        "- Use `--wait-for-index` or `build-index` when indexed servers return incomplete early results.",
        "- Fall back to plain file/content search when an LSP feature is unsupported or the result is obviously incomplete.",
    ]
    .join("\n")
}

fn render_known_values(config: &ConfigStore) -> String {
    let languages = config
        .filetypes
        .iter()
        .map(|filetype| filetype.id.as_str())
        .collect::<BTreeSet<_>>();
    let servers = config
        .lsps
        .iter()
        .map(|lsp| lsp.name.as_str())
        .collect::<BTreeSet<_>>();

    [
        "## Known values".to_string(),
        format!(
            "Configured languages in this installation: {}.",
            summarize_values(&languages.into_iter().collect::<Vec<_>>())
        ),
        format!(
            "Configured servers in this installation: {}.",
            summarize_values(&servers.into_iter().collect::<Vec<_>>())
        ),
        "If language or server selection is unclear, use `lsp-cli detect`, `lsp-cli languages`, or `lsp-cli servers --lang <LANG>` outside the main agent loop.".to_string(),
    ]
    .join("\n")
}

fn render_command_group(
    title: &str,
    specs: &[SkillCommandSpec],
    root: &ClapCommand,
) -> Result<String, String> {
    let mut sections = vec![title.to_string()];
    for spec in specs {
        let command = find_subcommand(root, spec.name)
            .ok_or_else(|| format!("failed to describe `{}`: command metadata is missing", spec.name))?;
        sections.push(render_command(spec, command));
    }
    Ok(sections.join("\n\n"))
}

fn render_command(spec: &SkillCommandSpec, command: &ClapCommand) -> String {
    let mut lines = vec![format!("### `{}`", spec.name)];
    lines.push(format!("Purpose: {}", command_about(command)));
    lines.push(format!("Use it when: {}", spec.when_to_use));
    lines.extend(spec.notes.iter().map(|note| format!("Note: {note}")));
    lines.push("Example:".to_string());
    lines.push("```sh".to_string());
    lines.push(spec.example.to_string());
    lines.push("```".to_string());

    if !spec.flags.is_empty() {
        lines.push("Recommended flags:".to_string());
        lines.extend(
            spec.flags
                .iter()
                .filter_map(|flag| find_arg(command, flag).map(format_arg_summary))
                .map(|summary| format!("- {summary}")),
        );
    }

    lines.join("\n")
}

fn render_limitations() -> String {
    [
        "## Limitations",
        "- Results are only as good as the selected LSP server.",
        "- Not every server supports every feature.",
        "- `workspace/symbol` quality and pattern syntax vary between servers.",
        "- Background indexing support varies, so `--wait-for-index` may help on some servers and do nothing on others.",
    ]
    .join("\n")
}

fn summarize_values(values: &[&str]) -> String {
    const MAX_ITEMS: usize = 12;

    if values.is_empty() {
        return "none".to_string();
    }

    if values.len() <= MAX_ITEMS {
        return values.join(", ");
    }

    format!(
        "{} and {} more",
        values[..MAX_ITEMS].join(", "),
        values.len() - MAX_ITEMS
    )
}

fn find_subcommand<'a>(root: &'a ClapCommand, name: &str) -> Option<&'a ClapCommand> {
    root.get_subcommands().find(|command| command.get_name() == name)
}

fn find_arg<'a>(command: &'a ClapCommand, long: &str) -> Option<&'a Arg> {
    command.get_arguments().find(|arg| arg.get_long() == Some(long))
}

fn command_about(command: &ClapCommand) -> String {
    command.get_about().map_or_else(
        || "No summary available.".to_string(),
        clap::builder::StyledStr::to_string,
    )
}

fn format_arg_summary(arg: &Arg) -> String {
    let mut label = String::new();
    if let Some(short) = arg.get_short() {
        label.push('-');
        label.push(short);
        if arg.get_long().is_some() {
            label.push_str(", ");
        }
    }
    if let Some(long) = arg.get_long() {
        label.push_str("--");
        label.push_str(long);
    }
    if arg.get_action().takes_values()
        && let Some(value_name) = arg.get_value_names().and_then(|names| names.first())
    {
        label.push(' ');
        label.push('<');
        label.push_str(value_name);
        label.push('>');
    }
    let help = arg.get_help().map_or_else(
        || "No help available.".to_string(),
        clap::builder::StyledStr::to_string,
    );
    format!("`{label}`: {help}")
}

struct SkillCommandSpec {
    name: &'static str,
    when_to_use: &'static str,
    example: &'static str,
    flags: &'static [&'static str],
    notes: &'static [&'static str],
}

const CORE_COMMANDS: &[SkillCommandSpec] = &[
    SkillCommandSpec {
        name: "grep",
        when_to_use: "you need semantic workspace symbol search before opening or editing files.",
        example: "lsp-cli grep --json --limit 20 Order path/to/project",
        flags: &["json", "limit", "wait-for-index", "detach", "files-with-matches"],
        notes: &["This uses `workspace/symbol`, so matching behavior depends on the server."],
    },
    SkillCommandSpec {
        name: "list-symbols",
        when_to_use: "you need a symbol outline for one file or a workspace slice.",
        example: "lsp-cli list-symbols --json --limit 50 path/to/project/src/main.rs",
        flags: &["json", "limit", "wait-for-index", "detach"],
        notes: &["Pass a file path for a focused outline or a directory for a broader scan."],
    },
    SkillCommandSpec {
        name: "list-functions",
        when_to_use: "you want a compact list of callable entry points in a workspace.",
        example: "lsp-cli list-functions --json --limit 50 path/to/project",
        flags: &["json", "limit", "wait-for-index", "detach"],
        notes: &["Useful for discovering candidate APIs before deeper navigation."],
    },
    SkillCommandSpec {
        name: "list-files",
        when_to_use: "you need the file set that the selected LSP workspace query will consider.",
        example: "lsp-cli list-files --json --limit 100 path/to/project",
        flags: &["json", "limit", "wait-for-index"],
        notes: &["Useful before diagnostics or workspace-wide symbol queries in mixed repositories."],
    },
    SkillCommandSpec {
        name: "definition",
        when_to_use: "you need the implementation location for a symbol before editing or reading code.",
        example: "lsp-cli definition --json --limit 10 MySymbol path/to/project",
        flags: &["json", "limit", "wait-for-index", "detach", "full", "files-with-matches"],
        notes: &["Use `--full` only when you need the returned source snippet, because it can expand output a lot."],
    },
    SkillCommandSpec {
        name: "declaration",
        when_to_use: "you need the declared API location rather than the implementation site.",
        example: "lsp-cli declaration --json --limit 10 MySymbol path/to/project",
        flags: &["json", "limit", "wait-for-index", "detach", "full", "files-with-matches"],
        notes: &["This is most useful in languages that distinguish declarations from definitions."],
    },
    SkillCommandSpec {
        name: "references",
        when_to_use: "you need impact analysis before a rename, signature change, or behavior change.",
        example: "lsp-cli references --json --limit 100 MySymbol path/to/project",
        flags: &["json", "limit", "wait-for-index", "detach", "files-with-matches"],
        notes: &["Prefer this before wide edits so the agent does not miss indirect usage sites."],
    },
    SkillCommandSpec {
        name: "callers",
        when_to_use: "you need to understand which code paths invoke a function.",
        example: "lsp-cli callers --json --limit 50 format_order path/to/project",
        flags: &["json", "limit", "wait-for-index", "detach", "files-with-matches"],
        notes: &["Use together with `callees` to sketch a local call graph."],
    },
    SkillCommandSpec {
        name: "callees",
        when_to_use: "you need to understand which functions a symbol depends on.",
        example: "lsp-cli callees --json --limit 50 format_order path/to/project",
        flags: &["json", "limit", "wait-for-index", "detach", "files-with-matches"],
        notes: &["This is useful for estimating side effects before touching a function body."],
    },
    SkillCommandSpec {
        name: "diagnostics",
        when_to_use: "you need LSP-reported errors and warnings after making edits.",
        example: "lsp-cli diagnostics --json --limit 100 path/to/project",
        flags: &["json", "limit", "wait-for-index", "detach", "files-with-matches"],
        notes: &["Use this after edits even when tests pass, because the language server may report unresolved symbols or type issues."],
    },
    SkillCommandSpec {
        name: "format",
        when_to_use: "you need language-server-native formatting or a formatting check before finishing an edit.",
        example: "lsp-cli format --check path/to/file.rs",
        flags: &["check", "stdout", "json", "detach"],
        notes: &["Use `--stdout` when the agent wants to inspect formatting changes before rewriting the file."],
    },
];

const SUPPORT_COMMANDS: &[SkillCommandSpec] = &[
    SkillCommandSpec {
        name: "server-capabilities",
        when_to_use: "you need to confirm whether the chosen server advertises a feature before relying on it.",
        example: "lsp-cli server-capabilities path/to/project --lsp rust-analyzer",
        flags: &["lang", "lsp", "detach", "download", "timeout"],
        notes: &["Use this when an empty result might mean unsupported capability rather than no matches."],
    },
    SkillCommandSpec {
        name: "build-index",
        when_to_use: "you expect an indexed server such as `clangd` or `rust-analyzer` to need warm-up time.",
        example: "lsp-cli build-index path/to/project --lsp rust-analyzer",
        flags: &["lang", "lsp", "detach", "download", "timeout"],
        notes: &["Prefer this before large workspace queries when early results look incomplete."],
    },
    SkillCommandSpec {
        name: "detect",
        when_to_use: "you are not sure which language or server `lsp-cli` will pick for a path.",
        example: "lsp-cli detect --json path/to/project",
        flags: &["lang", "lsp", "download", "json"],
        notes: &["This is a setup tool, not a main analysis command, so use it to resolve ambiguity and then switch back to semantic queries."],
    },
    SkillCommandSpec {
        name: "daemon",
        when_to_use: "you want to prewarm a server for many subsequent `--detach` requests.",
        example: "lsp-cli daemon path/to/project",
        flags: &["lang", "lsp", "download", "debug"],
        notes: &["This is optional because `--detach` can also spawn the background daemon on demand."],
    },
    SkillCommandSpec {
        name: "languages",
        when_to_use: "you need to discover the canonical language ids accepted by `--lang`.",
        example: "lsp-cli languages",
        flags: &[],
        notes: &["This is mainly useful when `detect` reports multiple languages or a guess needs to be forced manually."],
    },
    SkillCommandSpec {
        name: "servers",
        when_to_use: "you need to discover valid `--lsp` names, especially after narrowing to one language.",
        example: "lsp-cli servers --lang python",
        flags: &["lang"],
        notes: &["Use this to pick a different configured server when the default one behaves poorly."],
    },
];

#[cfg(test)]
mod tests {
    use super::{render_skill, run};
    use crate::cli::AgentSkillArgs;
    use crate::config::{CliConfig, ConfigStore, FiletypeConfig, LspConfig};
    use crate::test_support::TestDir;
    use regex::Regex;
    use std::path::PathBuf;

    fn config() -> ConfigStore {
        ConfigStore {
            filetypes: vec![
                FiletypeConfig {
                    id: "python".to_string(),
                    extensions: vec!["py".to_string()],
                    patterns: Vec::new(),
                },
                FiletypeConfig {
                    id: "rust".to_string(),
                    extensions: vec!["rs".to_string()],
                    patterns: vec![Regex::new("^Cargo\\.toml$").expect("regex should compile")],
                },
            ],
            lsps: vec![
                LspConfig {
                    id: "pyright".to_string(),
                    filetypes: vec!["python".to_string()],
                    root_markers: Vec::new(),
                    name: "pyright-langserver".to_string(),
                    cmdline: "pyright-langserver --stdio".to_string(),
                    wait_for_index: false,
                },
                LspConfig {
                    id: "rust_analyzer".to_string(),
                    filetypes: vec!["rust".to_string()],
                    root_markers: Vec::new(),
                    name: "rust-analyzer".to_string(),
                    cmdline: "rust-analyzer".to_string(),
                    wait_for_index: true,
                },
            ],
            cli: CliConfig::default(),
        }
    }

    #[test]
    fn renders_curated_skill_sections() {
        let markdown = render_skill(&config()).expect("skill should render");

        assert!(markdown.contains("# lsp-cli skill"));
        assert!(markdown.contains("## Core commands"));
        assert!(markdown.contains("### `grep`"));
        assert!(markdown.contains("### `definition`"));
        assert!(markdown.contains("### `detect`"));
        assert!(markdown.contains("Configured languages in this installation: python, rust."));
        assert!(markdown.contains("Configured servers in this installation: pyright-langserver, rust-analyzer."));
        assert!(markdown.contains("Prefer `--json`"));
    }

    #[test]
    fn excludes_plumbing_commands_from_generated_skill() {
        let markdown = render_skill(&config()).expect("skill should render");

        assert!(!markdown.contains("### `completion`"));
        assert!(!markdown.contains("### `run`"));
        assert!(!markdown.contains("### `commands`"));
        assert!(!markdown.contains("### `stop`"));
        assert!(!markdown.contains("### `stop-all`"));
        assert!(!markdown.contains("### `update`"));
    }

    #[test]
    fn writes_skill_file_to_requested_path() {
        let dir = TestDir::new("agent-skill");
        let path = dir.path().join("docs/SKILL.md");

        let output = run(
            &AgentSkillArgs { path: path.clone() },
            &config(),
        )
        .expect("skill file should be written");

        assert_eq!(output, format!("wrote agent skill to {}", path.display()));
        let markdown = std::fs::read_to_string(&path).expect("skill file should be readable");
        assert!(markdown.contains("# lsp-cli skill"));
    }

    #[test]
    fn renders_boolean_flags_without_placeholder_values() {
        let markdown = render_skill(&config()).expect("skill should render");

        assert!(markdown.contains("`--json`: Print results as JSON."));
        assert!(!markdown.contains("`--json <JSON>`"));
    }

    #[test]
    fn dash_path_writes_to_stdout() {
        let markdown = run(
            &AgentSkillArgs {
                path: PathBuf::from("-"),
            },
            &config(),
        )
        .expect("skill should render to stdout");

        assert!(markdown.contains("# lsp-cli skill"));
        assert!(!markdown.contains("wrote agent skill"));
    }
}
