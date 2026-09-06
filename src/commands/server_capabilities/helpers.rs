use std::{env, fs};

use serde_json::Value;

use super::{InitializeResponse, KNOWN_TOP_LEVEL_KEYS, render_generic_capability};

pub(super) fn render_unknown_capabilities(
    initialize: &InitializeResponse,
    lines: &mut Vec<String>,
) {
    let Some(capabilities) = initialize.capabilities_raw().and_then(Value::as_object) else {
        return;
    };

    for key in sorted_keys(capabilities) {
        if KNOWN_TOP_LEVEL_KEYS.contains(&key.as_str()) {
            continue;
        }
        if let Some(value) = capabilities.get(key.as_str()) {
            render_generic_capability(key.as_str(), Some(value), 2, lines);
        }
    }

    if let Some(experimental) = capabilities.get("experimental") {
        render_experimental("experimental", experimental, lines);
    }
}

fn render_experimental(prefix: &str, value: &Value, lines: &mut Vec<String>) {
    if let Value::Object(map) = value {
        if map.is_empty() {
            render_generic_capability(prefix, Some(value), 2, lines);
            return;
        }

        for key in sorted_keys(map) {
            if let Some(child) = map.get(key.as_str()) {
                render_experimental(&format!("{prefix}/{key}"), child, lines);
            }
        }
        return;
    }

    render_generic_capability(prefix, Some(value), 2, lines);
}

pub(super) fn sorted_keys(map: &serde_json::Map<String, Value>) -> Vec<&String> {
    let mut keys = map.keys().collect::<Vec<_>>();
    keys.sort();
    keys
}

pub(super) fn display_command(command: &[String]) -> String {
    let Some((program, args)) = command.split_first() else {
        return String::new();
    };
    std::iter::once(resolve_program_path(program))
        .chain(args.iter().map(|argument| shell_escape(argument)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve_program_path(program: &str) -> String {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return shell_escape(program);
    }

    let Some(path) = env::var_os("PATH") else {
        return shell_escape(program);
    };
    for entry in env::split_paths(&path) {
        let candidate = entry.join(program);
        if fs::metadata(&candidate).is_ok_and(|metadata| metadata.is_file()) {
            return shell_escape(candidate.display().to_string().as_str());
        }
    }

    shell_escape(program)
}

fn shell_escape(value: &str) -> String {
    if !value.contains([' ', '\t', '\n', '\'', '"']) {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
