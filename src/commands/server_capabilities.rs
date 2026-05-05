use crate::cli::ServerCapabilitiesArgs;
use crate::commands::common::{connect_lsp_client, prepare_workspace};
use crate::config::ConfigStore;
use crate::lsp::InitializeResponse;
use serde_json::Value;
use std::env;
use std::fs;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
enum RenderMode {
    Generic,
    PositionEncoding,
    TextDocumentSync,
    WorkspaceFolders,
}

#[derive(Clone, Copy)]
struct CapabilitySpec {
    pretty: &'static str,
    raw_path: &'static [&'static str],
    mode: RenderMode,
}

const CAPABILITY_SPECS: &[CapabilitySpec] = &[
    CapabilitySpec {
        pretty: "position encoding",
        raw_path: &["positionEncoding"],
        mode: RenderMode::PositionEncoding,
    },
    CapabilitySpec {
        pretty: "text document sync",
        raw_path: &["textDocumentSync"],
        mode: RenderMode::TextDocumentSync,
    },
    CapabilitySpec {
        pretty: "notebook document sync",
        raw_path: &["notebookDocumentSync"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "selection ranges",
        raw_path: &["selectionRangeProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "hover",
        raw_path: &["hoverProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "completion",
        raw_path: &["completionProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "signature help",
        raw_path: &["signatureHelpProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "goto definition",
        raw_path: &["definitionProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "goto type definition",
        raw_path: &["typeDefinitionProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "goto implementation",
        raw_path: &["implementationProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "find references",
        raw_path: &["referencesProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "document highlights",
        raw_path: &["documentHighlightProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "document symbols",
        raw_path: &["documentSymbolProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "workspace symbols",
        raw_path: &["workspaceSymbolProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "code actions",
        raw_path: &["codeActionProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "code lens",
        raw_path: &["codeLensProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "document formatting",
        raw_path: &["documentFormattingProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "document range formatting",
        raw_path: &["documentRangeFormattingProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "document on-type formatting",
        raw_path: &["documentOnTypeFormattingProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "rename",
        raw_path: &["renameProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "document links",
        raw_path: &["documentLinkProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "document color",
        raw_path: &["colorProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "folding ranges",
        raw_path: &["foldingRangeProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "goto declaration",
        raw_path: &["declarationProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "execute command",
        raw_path: &["executeCommandProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "workspace folders",
        raw_path: &["workspace", "workspaceFolders"],
        mode: RenderMode::WorkspaceFolders,
    },
    CapabilitySpec {
        pretty: "workspace file create notifications",
        raw_path: &["workspace", "fileOperations", "didCreate"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "workspace file create preparation",
        raw_path: &["workspace", "fileOperations", "willCreate"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "workspace file rename notifications",
        raw_path: &["workspace", "fileOperations", "didRename"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "workspace file rename preparation",
        raw_path: &["workspace", "fileOperations", "willRename"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "workspace file delete notifications",
        raw_path: &["workspace", "fileOperations", "didDelete"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "workspace file delete preparation",
        raw_path: &["workspace", "fileOperations", "willDelete"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "call hierarchy",
        raw_path: &["callHierarchyProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "semantic tokens",
        raw_path: &["semanticTokensProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "monikers",
        raw_path: &["monikerProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "linked editing ranges",
        raw_path: &["linkedEditingRangeProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "inline values",
        raw_path: &["inlineValueProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "inlay hints",
        raw_path: &["inlayHintProvider"],
        mode: RenderMode::Generic,
    },
    CapabilitySpec {
        pretty: "pull diagnostics",
        raw_path: &["diagnosticProvider"],
        mode: RenderMode::Generic,
    },
];

const KNOWN_TOP_LEVEL_KEYS: &[&str] = &[
    "positionEncoding",
    "textDocumentSync",
    "notebookDocumentSync",
    "selectionRangeProvider",
    "hoverProvider",
    "completionProvider",
    "signatureHelpProvider",
    "definitionProvider",
    "typeDefinitionProvider",
    "implementationProvider",
    "referencesProvider",
    "documentHighlightProvider",
    "documentSymbolProvider",
    "workspaceSymbolProvider",
    "codeActionProvider",
    "codeLensProvider",
    "documentFormattingProvider",
    "documentRangeFormattingProvider",
    "documentOnTypeFormattingProvider",
    "renameProvider",
    "documentLinkProvider",
    "colorProvider",
    "foldingRangeProvider",
    "declarationProvider",
    "executeCommandProvider",
    "workspace",
    "callHierarchyProvider",
    "semanticTokensProvider",
    "monikerProvider",
    "linkedEditingRangeProvider",
    "inlineValueProvider",
    "inlayHintProvider",
    "diagnosticProvider",
    "experimental",
];

const FIELD_LABELS: &[(&str, &str)] = &[
    ("workDoneProgress", "work done progress"),
    ("resolveProvider", "resolve"),
    ("triggerCharacters", "trigger characters"),
    ("retriggerCharacters", "retrigger characters"),
    ("allCommitCharacters", "all commit characters"),
    ("completionItem", "completion item"),
    ("labelDetailsSupport", "label details support"),
    ("prepareProvider", "prepare rename"),
    ("commands", "commands"),
    ("openClose", "open/close"),
    ("change", "change"),
    ("willSave", "will save"),
    ("willSaveWaitUntil", "will save wait until"),
    ("save", "save"),
    ("includeText", "include text on save"),
    ("interFileDependencies", "inter-file dependencies"),
    ("workspaceDiagnostics", "workspace diagnostics"),
    ("identifier", "identifier"),
    ("firstTriggerCharacter", "first trigger character"),
    ("moreTriggerCharacter", "more trigger characters"),
    ("changeNotifications", "change notifications"),
    ("supported", "supported"),
    ("legend", "legend"),
    ("tokenTypes", "token types"),
    ("tokenModifiers", "token modifiers"),
    ("range", "range"),
    ("full", "full"),
    ("delta", "delta"),
    ("documentSelector", "document selector"),
    ("notebookSelector", "notebook selector"),
    ("filters", "file filters"),
    ("scheme", "scheme"),
    ("pattern", "pattern"),
    ("glob", "glob"),
    ("matches", "matches"),
    ("options", "options"),
    ("ignoreCase", "ignore case"),
    ("value", "value"),
    ("label", "label"),
    ("id", "id"),
    ("notebookType", "notebook type"),
    ("language", "language"),
];

pub(super) fn run(args: &ServerCapabilitiesArgs, config: &ConfigStore) -> Result<String, String> {
    // Q: args.server is duplicated
    let workspace = prepare_workspace(
        &args.directory,
        args.server.selected_server(),
        args.server.selected_language(),
        args.server.download,
        config,
    )?;
    let mut client =
        connect_lsp_client(&workspace, args.detach, args.server.debug, args.timeout)?;
    let initialize = client
        .initialize(&workspace.root_uri, &workspace.workspace_name, false)
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;
    let output = render_output(
        &workspace.server.command,
        &workspace.server.server,
        &initialize,
    );
    client.shutdown().map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;
    Ok(output)
}

fn render_output(
    command: &[String],
    fallback_server_name: &str,
    initialize: &InitializeResponse,
) -> String {
    let server_name = initialize
        .server_info()
        .map_or(fallback_server_name, |info| info.name.as_str());
    let server = initialize.server_info().map_or_else(
        || server_name.to_string(),
        |info| match info.version.as_deref() {
            Some(version) => format!("{server_name} ({version})"),
            None => server_name.to_string(),
        },
    );
    let mut lines = vec![
        format!("cmdline: {}", display_command(command)),
        format!("server: {server}"),
        "capabilities:".to_string(),
    ];

    for spec in CAPABILITY_SPECS {
        match spec.mode {
            RenderMode::Generic => render_generic_capability(
                spec.pretty,
                initialize.capability(spec.raw_path),
                2,
                &mut lines,
            ),
            RenderMode::PositionEncoding => {
                render_position_encoding(initialize.capability(spec.raw_path), 2, &mut lines);
            }
            RenderMode::TextDocumentSync => {
                render_text_document_sync(initialize.capability(spec.raw_path), 2, &mut lines);
            }
            RenderMode::WorkspaceFolders => {
                render_workspace_folders(initialize.capability(spec.raw_path), 2, &mut lines);
            }
        }
    }

    render_unknown_capabilities(initialize, &mut lines);
    lines.join("\n")
}

fn render_position_encoding(value: Option<&Value>, indent: usize, lines: &mut Vec<String>) {
    match value {
        Some(Value::Bool(false)) => push_line(lines, indent, "position encoding", "no"),
        Some(Value::String(encoding)) => push_line(lines, indent, "position encoding", encoding),
        None => push_line(lines, indent, "position encoding", "utf-16"),
        Some(other) => {
            push_line(lines, indent, "position encoding", "yes");
            render_value_details(other, indent + 2, lines);
        }
    }
}

fn render_text_document_sync(value: Option<&Value>, indent: usize, lines: &mut Vec<String>) {
    match value {
        None => push_line(lines, indent, "text document sync", "no"),
        Some(Value::Number(number)) => {
            let kind = sync_kind(number.as_i64());
            push_line(
                lines,
                indent,
                "text document sync",
                if kind == "none" { "no" } else { "yes" },
            );
            push_line(lines, indent + 2, "change", kind);
        }
        Some(Value::Object(map)) => {
            push_line(lines, indent, "text document sync", "yes");
            if let Some(value) = map.get("openClose") {
                render_named_value("open/close", value, indent + 2, lines);
            }
            if let Some(value) = map.get("change") {
                match value {
                    Value::Number(number) => {
                        push_line(lines, indent + 2, "change", sync_kind(number.as_i64()));
                    }
                    _ => render_named_value("change", value, indent + 2, lines),
                }
            }
            if let Some(value) = map.get("willSave") {
                render_named_value("will save", value, indent + 2, lines);
            }
            if let Some(value) = map.get("willSaveWaitUntil") {
                render_named_value("will save wait until", value, indent + 2, lines);
            }
            if let Some(value) = map.get("save") {
                render_named_value("save", value, indent + 2, lines);
            }

            for key in sorted_keys(map) {
                if [
                    "openClose",
                    "change",
                    "willSave",
                    "willSaveWaitUntil",
                    "save",
                ]
                .contains(&key.as_str())
                {
                    continue;
                }
                if let Some(value) = map.get(key.as_str()) {
                    render_named_value(pretty_key(key.as_str()), value, indent + 2, lines);
                }
            }
        }
        Some(other) => {
            push_line(lines, indent, "text document sync", "yes");
            render_value_details(other, indent + 2, lines);
        }
    }
}

fn render_workspace_folders(value: Option<&Value>, indent: usize, lines: &mut Vec<String>) {
    let Some(value) = value else {
        push_line(lines, indent, "workspace folders", "no");
        return;
    };
    let status = value
        .as_object()
        .and_then(|map| map.get("supported"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    push_line(
        lines,
        indent,
        "workspace folders",
        if status { "yes" } else { "no" },
    );
    if let Some(map) = value.as_object() {
        for key in sorted_keys(map) {
            if let Some(field) = map.get(key.as_str()) {
                render_named_value(pretty_key(key.as_str()), field, indent + 2, lines);
            }
        }
    } else {
        render_value_details(value, indent + 2, lines);
    }
}

fn render_generic_capability(
    name: &str,
    value: Option<&Value>,
    indent: usize,
    lines: &mut Vec<String>,
) {
    match value {
        None | Some(Value::Bool(false)) => push_line(lines, indent, name, "no"),
        Some(Value::Bool(true)) => push_line(lines, indent, name, "yes"),
        Some(other) => {
            push_line(lines, indent, name, "yes");
            render_value_details(other, indent + 2, lines);
        }
    }
}

fn render_value_details(value: &Value, indent: usize, lines: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for key in sorted_keys(map) {
                if let Some(field) = map.get(key.as_str()) {
                    render_named_value(pretty_key(key.as_str()), field, indent, lines);
                }
            }
        }
        Value::Null => push_line(lines, indent, "value", "null"),
        other => push_line(lines, indent, "value", &format_scalar(other)),
    }
}

fn render_named_value(name: &str, value: &Value, indent: usize, lines: &mut Vec<String>) {
    match value {
        Value::Bool(true) => push_line(lines, indent, name, "yes"),
        Value::Bool(false) => push_line(lines, indent, name, "no"),
        Value::Object(map) => {
            push_line(lines, indent, name, "yes");
            for key in sorted_keys(map) {
                if let Some(field) = map.get(key.as_str()) {
                    render_named_value(pretty_key(key.as_str()), field, indent + 2, lines);
                }
            }
        }
        Value::Null => push_line(lines, indent, name, "null"),
        other => push_line(lines, indent, name, &format_scalar(other)),
    }
}

fn render_unknown_capabilities(initialize: &InitializeResponse, lines: &mut Vec<String>) {
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

fn sorted_keys(map: &serde_json::Map<String, Value>) -> Vec<&String> {
    let mut keys = map.keys().collect::<Vec<_>>();
    keys.sort();
    keys
}

fn pretty_key(key: &str) -> &str {
    FIELD_LABELS
        .iter()
        .find_map(|(raw, pretty)| (*raw == key).then_some(*pretty))
        .unwrap_or(key)
}

fn sync_kind(value: Option<i64>) -> &'static str {
    match value {
        Some(0) => "none",
        Some(1) => "full",
        Some(2) => "incremental",
        _ => "unknown",
    }
}

fn push_line(lines: &mut Vec<String>, indent: usize, name: &str, value: &str) {
    lines.push(format!("{}{}: {value}", " ".repeat(indent), name));
}

fn format_scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items)
            if items
                .iter()
                .all(|item| !item.is_object() && !item.is_array()) =>
        {
            items
                .iter()
                .map(format_scalar)
                .collect::<Vec<_>>()
                .join(", ")
        }
        _ => value.to_string(),
    }
}

fn display_command(command: &[String]) -> String {
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

#[cfg(test)]
pub(super) fn render_for_tests(
    command: &[String],
    fallback_server_name: &str,
    initialize: &InitializeResponse,
) -> String {
    render_output(command, fallback_server_name, initialize)
}
