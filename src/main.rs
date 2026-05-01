#![warn(clippy::pedantic)]

mod cli;
mod config;
mod detect;
mod lsp;
mod suggest;

use std::collections::{BTreeSet, HashMap};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Cursor;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;

use clap_complete::generate;
use cli::{
    BuildIndexArgs, Command as CliCommand, CompletionArgs, DetectArgs, GrepArgs, ListSymbolsArgs,
    RunArgs, WorkspaceQueryArgs, clap_command, parse_args,
};
use config::{ConfigStore, default_config_root, load_config_store};
use detect::{DetectionResult, detect_workspace};
use lsp::{InitializeResponse, LspClient};
use serde::Deserialize;
use serde_json::{Value, json};
use suggest::{SuggestedLanguage, suggestions_for};
use url::Url;

#[derive(Debug, Eq, PartialEq)]
struct SymbolMatch {
    path: PathBuf,
    line: u32,
    col: u32,
    line_content: String,
}

struct PreparedWorkspace {
    detection: DetectionResult,
    server: SuggestedLanguage,
    root_uri: String,
    workspace_name: String,
}

struct WorkspaceSymbolQueryResult {
    detected_filetypes: BTreeSet<String>,
    server: SuggestedLanguage,
    matches: Vec<SymbolMatch>,
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            process::exit(2);
        }
    };

    let args = match args {
        CliCommand::Completion(args) => {
            match generate_completion(args) {
                Ok(output) => print!("{output}"),
                Err(error) => {
                    eprintln!("failed to generate completion: {error}");
                    process::exit(1);
                }
            }
            return;
        }
        args => args,
    };

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

    let output = match args {
        CliCommand::Detect(args) => run_detect(&args, &config),
        CliCommand::Grep(args) => run_grep(&args, &config),
        CliCommand::ListSymbols(args) => run_list_symbols(&args, &config),
        CliCommand::BuildIndex(args) => run_build_index(&args, &config),
        CliCommand::Completion(_) => unreachable!("completion handled before config loading"),
        CliCommand::Run(args) => run_run(&args, &config),
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

fn generate_completion(args: CompletionArgs) -> Result<String, String> {
    let shell = args.shell.map_or_else(detect_current_shell, Ok)?;
    let mut command = clap_command();
    let mut output = Cursor::new(Vec::new());
    generate(shell, &mut command, "lsp-cli", &mut output);

    String::from_utf8(output.into_inner())
        .map_err(|error| format!("completion output was not valid UTF-8: {error}"))
}

fn detect_current_shell() -> Result<clap_complete::Shell, String> {
    clap_complete::Shell::from_env()
        .ok_or(())
        .or_else(|()| detect_shell_from_env(env::var_os("SHELL").as_deref()))
}

fn detect_shell_from_env(shell: Option<&OsStr>) -> Result<clap_complete::Shell, String> {
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

fn run_detect(args: &DetectArgs, config: &ConfigStore) -> Result<String, String> {
    let (detection, suggestions) = analyze_path(&args.path, config)?;

    Ok(if args.json {
        render_detect_json(&suggestions)
    } else if args.quiet {
        render_detect_quiet(&suggestions)
    } else {
        render_detect_text(&detection.filetypes, &suggestions)
    })
}

fn run_grep(args: &GrepArgs, config: &ConfigStore) -> Result<String, String> {
    let result = run_workspace_symbol_query(&args.query, &args.pattern, config)?;

    Ok(if args.query.json {
        render_workspace_symbol_json(
            &args.pattern,
            &args.query.directory,
            &result.detected_filetypes,
            &result.server,
            &result.matches,
        )
    } else {
        render_symbol_matches_text(&result.matches)
    })
}

fn run_build_index(args: &BuildIndexArgs, config: &ConfigStore) -> Result<String, String> {
    let workspace = prepare_workspace(&args.directory, args.lsp.as_deref(), config)?;

    let mut client = LspClient::new(&workspace.server.command, args.debug, args.timeout)?;
    client
        .initialize(&workspace.root_uri, &workspace.workspace_name, true)
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;

    let wait = client.wait_for_background_work();
    let shutdown = client.shutdown();
    wait.map_err(|error| {
        format!(
            "failed to build index with {}: {error}",
            workspace.server.server
        )
    })?;
    shutdown.map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;

    Ok(String::new())
}

fn run_list_symbols(args: &ListSymbolsArgs, config: &ConfigStore) -> Result<String, String> {
    let result = run_workspace_symbol_query(&args.query, "", config)?;

    Ok(if args.query.json {
        render_workspace_symbol_json(
            "",
            &args.query.directory,
            &result.detected_filetypes,
            &result.server,
            &result.matches,
        )
    } else {
        render_symbol_matches_text(&result.matches)
    })
}

fn run_run(args: &RunArgs, config: &ConfigStore) -> Result<String, String> {
    let (detection, suggestions) = analyze_path(&args.path, config)?;
    let server = select_server(&detection, &suggestions, args.lsp.as_deref())?;
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
        .current_dir(&server.workspace_root);

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

fn ensure_workspace_symbol_support(initialize: &InitializeResponse) -> Result<(), String> {
    if matches!(
        initialize.capabilities.workspace_symbol_provider,
        Some(Value::Bool(false)) | None
    ) {
        return Err("selected LSP server does not support workspace/symbol".to_string());
    }

    Ok(())
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

fn analyze_path(
    path: &Path,
    config: &ConfigStore,
) -> Result<(DetectionResult, Vec<SuggestedLanguage>), String> {
    let detection = detect_workspace(path, &config.filetypes)
        .map_err(|error| format!("failed to scan {}: {error}", path.display()))?;
    let suggestions = suggestions_for(&config.lsps, &detection, path)
        .map_err(|error| format!("failed to build suggestions: {error}"))?;

    Ok((detection, suggestions))
}

fn prepare_workspace(
    path: &Path,
    selected_server: Option<&str>,
    config: &ConfigStore,
) -> Result<PreparedWorkspace, String> {
    let (detection, suggestions) = analyze_path(path, config)?;
    let server = select_server(&detection, &suggestions, selected_server)?.clone();
    let root_uri = path_to_file_uri(&server.workspace_root)?;
    let workspace_name = lsp::workspace_name(&server.workspace_root);

    Ok(PreparedWorkspace {
        detection,
        server,
        root_uri,
        workspace_name,
    })
}

fn run_workspace_symbol_query(
    args: &WorkspaceQueryArgs,
    query: &str,
    config: &ConfigStore,
) -> Result<WorkspaceSymbolQueryResult, String> {
    let workspace = prepare_workspace(&args.directory, args.lsp.as_deref(), config)?;
    let wait_for_index = args.wait_for_index || workspace.server.wait_for_index;

    let mut client = LspClient::new(&workspace.server.command, args.debug, args.timeout)?;
    let initialize = client
        .initialize(
            &workspace.root_uri,
            &workspace.workspace_name,
            wait_for_index,
        )
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;
    ensure_workspace_symbol_support(&initialize)?;

    let response = (if wait_for_index {
        client.wait_for_background_work().map_err(|error| {
            format!(
                "failed to wait for background work with {}: {error}",
                workspace.server.server
            )
        })
    } else {
        Ok(())
    })
    .and_then(|()| {
        client
            .workspace_symbol(query)
            .map_err(|error| format!("failed to query {}: {error}", workspace.server.server))
    });
    let shutdown = client.shutdown();
    let response = response?;
    shutdown.map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;

    Ok(WorkspaceSymbolQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        matches: symbol_matches_from_response(&response)?,
    })
}

fn select_server<'a>(
    detection: &DetectionResult,
    suggestions: &'a [SuggestedLanguage],
    selected_server: Option<&str>,
) -> Result<&'a SuggestedLanguage, String> {
    if let Some(server) = selected_server {
        return suggestions.iter().find(|suggestion| suggestion.server == server).ok_or_else(|| {
            let available = suggestions
                .iter()
                .map(|suggestion| suggestion.server.as_str())
                .collect::<Vec<_>>();
            if available.is_empty() {
                format!("Requested LSP server {server:?} is not available because no matching servers were detected")
            } else {
                format!(
                    "Requested LSP server {server:?} is not in the detected server list: {}",
                    available.join(", ")
                )
            }
        });
    }

    suggestions.first().ok_or_else(|| {
        if detection.filetypes.is_empty() {
            "No supported languages detected".to_string()
        } else {
            format!(
                "No LSP server matches detected filetypes: {}",
                detection
                    .filetypes
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    })
}

fn render_detect_quiet(suggestions: &[SuggestedLanguage]) -> String {
    suggestions
        .iter()
        .map(|suggestion| suggestion.command.join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_detect_text(
    detected_filetypes: &BTreeSet<String>,
    suggestions: &[SuggestedLanguage],
) -> String {
    if suggestions.is_empty() {
        return "No supported languages detected".to_string();
    }

    let detected = if detected_filetypes.is_empty() {
        "none".to_string()
    } else {
        detected_filetypes
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };

    suggestions
        .iter()
        .map(|suggestion| {
            format!(
                "Detected: {}\nSuggested command: {}",
                detected,
                suggestion.command.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_detect_json(suggestions: &[SuggestedLanguage]) -> String {
    json!({
        "servers": suggestions
            .iter()
            .map(|suggestion| {
                json!({
                    "languages": suggestion.languages,
                    "server": suggestion.server,
                    "command": suggestion.command,
                })
            })
            .collect::<Vec<_>>(),
    })
    .to_string()
}

fn render_symbol_matches_text(matches: &[SymbolMatch]) -> String {
    matches
        .iter()
        .map(|matched| {
            format!(
                "{}:{}:{}:{}",
                matched.path.display(),
                matched.line,
                matched.col,
                matched.line_content
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_workspace_symbol_json(
    query: &str,
    directory: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[SymbolMatch],
) -> String {
    json!({
        "query": query,
        "directory": directory,
        "detected": detected_filetypes,
        "server": render_server_json(server),
        "matches": render_symbol_matches_json(matches),
    })
    .to_string()
}

fn render_server_json(server: &SuggestedLanguage) -> Value {
    json!({
        "name": server.server,
        "languages": server.languages,
        "command": server.command,
        "workspace_root": server.workspace_root,
    })
}

fn render_symbol_matches_json(matches: &[SymbolMatch]) -> Vec<Value> {
    matches
        .iter()
        .map(|matched| {
            json!({
                "path": matched.path,
                "line": matched.line,
                "col": matched.col,
                "line_content": matched.line_content,
            })
        })
        .collect()
}

fn symbol_matches_from_response(response: &Value) -> Result<Vec<SymbolMatch>, String> {
    if response.is_null() {
        return Ok(Vec::new());
    }

    let symbols: Vec<WorkspaceSymbolItem> = serde_json::from_value(response.clone())
        .map_err(|error| format!("failed to decode workspace/symbol response: {error}"))?;
    let mut source_cache = SourceCache::default();

    symbols
        .into_iter()
        .filter_map(|symbol| symbol.into_symbol_match(&mut source_cache))
        .collect()
}

fn path_to_file_uri(path: &Path) -> Result<String, String> {
    let absolute = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {}: {error}", path.display()))?;

    let url = if absolute.is_dir() {
        Url::from_directory_path(&absolute)
    } else {
        Url::from_file_path(&absolute)
    }
    .map_err(|()| format!("failed to build file URI for {}", absolute.display()))?;

    Ok(url.to_string())
}

#[derive(Debug, Default)]
struct SourceCache {
    lines: HashMap<PathBuf, Vec<String>>,
}

impl SourceCache {
    fn line_content(&mut self, path: &Path, line_index: usize) -> String {
        let entry = self.lines.entry(path.to_path_buf()).or_insert_with(|| {
            fs::read_to_string(path)
                .map(|contents| contents.lines().map(ToString::to_string).collect())
                .unwrap_or_default()
        });

        entry
            .get(line_index)
            .cloned()
            .unwrap_or_else(|| "<line unavailable>".to_string())
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkspaceSymbolItem {
    SymbolInformation(SymbolInformationItem),
    WorkspaceSymbol(WorkspaceSymbol),
}

impl WorkspaceSymbolItem {
    fn into_symbol_match(
        self,
        source_cache: &mut SourceCache,
    ) -> Option<Result<SymbolMatch, String>> {
        match self {
            Self::SymbolInformation(symbol) => {
                Some(symbol.location.into_symbol_match(source_cache))
            }
            Self::WorkspaceSymbol(symbol) => symbol.location.into_symbol_match(source_cache),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SymbolInformationItem {
    location: Location,
}

#[derive(Debug, Deserialize)]
struct WorkspaceSymbol {
    location: WorkspaceSymbolLocation,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WorkspaceSymbolLocation {
    Full(Location),
    UriOnly {
        #[serde(rename = "uri")]
        _uri: Value,
    },
}

impl WorkspaceSymbolLocation {
    fn into_symbol_match(
        self,
        source_cache: &mut SourceCache,
    ) -> Option<Result<SymbolMatch, String>> {
        match self {
            Self::Full(location) => Some(location.into_symbol_match(source_cache)),
            Self::UriOnly { .. } => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Location {
    uri: String,
    range: Range,
}

impl Location {
    fn into_symbol_match(self, source_cache: &mut SourceCache) -> Result<SymbolMatch, String> {
        let path = file_uri_to_path(&self.uri)?;
        let line = self.range.start.line + 1;
        let col = self.range.start.character + 1;
        let line_index = usize::try_from(self.range.start.line)
            .map_err(|_| format!("line index overflow for {}", path.display()))?;
        let line_content = source_cache.line_content(&path, line_index);

        Ok(SymbolMatch {
            path,
            line,
            col,
            line_content,
        })
    }
}

#[derive(Debug, Deserialize)]
struct Range {
    start: Position,
}

#[derive(Debug, Deserialize)]
struct Position {
    line: u32,
    character: u32,
}

fn file_uri_to_path(uri: &str) -> Result<PathBuf, String> {
    let url = Url::parse(uri).map_err(|error| format!("invalid location URI {uri:?}: {error}"))?;

    url.to_file_path()
        .map_err(|()| format!("workspace/symbol returned non-file URI {uri:?}"))
}

#[cfg(test)]
mod tests {
    use super::{
        SourceCache, SymbolMatch, detect_shell_from_env, generate_completion, render_detect_json,
        render_detect_quiet, render_detect_text, render_symbol_matches_text, select_server,
        symbol_matches_from_response,
    };
    use crate::cli::CompletionArgs;
    use crate::detect::DetectionResult;
    use crate::suggest::SuggestedLanguage;
    use clap_complete::Shell;
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use url::Url;

    fn example_suggestion() -> SuggestedLanguage {
        SuggestedLanguage {
            languages: vec!["alpha".to_string(), "beta".to_string()],
            server: "example-lsp".to_string(),
            command: vec!["example-lsp".to_string(), "--stdio".to_string()],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
        }
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lsp-cli-main-test-{}-{}",
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).expect("temp dir should be created");

            Self { path }
        }

        fn write_file(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dirs should be created");
            }

            fs::write(&path, contents).expect("file should be written");
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn renders_empty_detect_text_output() {
        assert_eq!(
            render_detect_text(&BTreeSet::new(), &[]),
            "No supported languages detected"
        );
    }

    #[test]
    fn renders_detect_text_output() {
        let detected = BTreeSet::from(["alpha".to_string(), "beta".to_string()]);

        assert_eq!(
            render_detect_text(&detected, &[example_suggestion()]),
            "Detected: alpha, beta\nSuggested command: example-lsp --stdio"
        );
    }

    #[test]
    fn renders_detect_quiet_output() {
        assert_eq!(
            render_detect_quiet(&[example_suggestion()]),
            "example-lsp --stdio"
        );
    }

    #[test]
    fn renders_detect_json_output() {
        assert_eq!(
            render_detect_json(&[example_suggestion()]),
            concat!(
                "{\"servers\":[",
                "{\"command\":[\"example-lsp\",\"--stdio\"],\"languages\":[\"alpha\",\"beta\"],\"server\":\"example-lsp\"}",
                "]}"
            )
        );
    }

    #[test]
    fn renders_grep_text_output() {
        assert_eq!(
            render_symbol_matches_text(&[SymbolMatch {
                path: PathBuf::from("src/main.rs"),
                line: 3,
                col: 14,
                line_content: "fn main() {".to_string(),
            }]),
            "src/main.rs:3:14:fn main() {"
        );
    }

    #[test]
    fn renders_empty_grep_text_output() {
        assert_eq!(render_symbol_matches_text(&[]), "");
    }

    #[test]
    fn returns_placeholder_for_missing_line() {
        let dir = TestDir::new();
        let file = dir.write_file("main.rs", "fn main() {}\n");
        let mut cache = SourceCache::default();

        assert_eq!(cache.line_content(&file, 99), "<line unavailable>");
    }

    #[test]
    fn parses_workspace_symbol_locations() {
        let dir = TestDir::new();
        let file = dir.write_file("src/lib.rs", "first line\nsecond line\n");
        let uri = Url::from_file_path(&file)
            .expect("file path should become URI")
            .to_string();

        let matches = symbol_matches_from_response(&json!([
            {
                "name": "symbol",
                "kind": 12,
                "location": {
                    "uri": uri,
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": 8 }
                    }
                }
            }
        ]))
        .expect("response should parse");

        assert_eq!(
            matches,
            vec![SymbolMatch {
                path: file,
                line: 2,
                col: 3,
                line_content: "second line".to_string(),
            }]
        );
    }

    #[test]
    fn selects_requested_server_for_grep() {
        let primary = example_suggestion();
        let secondary = SuggestedLanguage {
            languages: vec!["beta".to_string()],
            server: "secondary-lsp".to_string(),
            command: vec!["secondary-lsp".to_string()],
            workspace_root: PathBuf::from("."),
            wait_for_index: false,
        };
        let suggestions = [primary, secondary.clone()];

        let selected = select_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            &suggestions,
            Some("secondary-lsp"),
        )
        .expect("requested server should be selected");

        assert_eq!(selected.server, secondary.server);
    }

    #[test]
    fn errors_when_requested_server_is_not_detected() {
        let error = select_server(
            &DetectionResult {
                filetypes: BTreeSet::from(["beta".to_string()]),
                filenames: BTreeSet::new(),
            },
            &[example_suggestion()],
            Some("missing-lsp"),
        )
        .expect_err("missing server should error");

        assert_eq!(
            error,
            "Requested LSP server \"missing-lsp\" is not in the detected server list: example-lsp"
        );
    }

    #[test]
    fn generates_bash_completion_script() {
        let output = generate_completion(CompletionArgs {
            shell: Some(Shell::Bash),
        })
        .expect("completion script should generate");

        assert!(output.contains("lsp-cli"));
        assert!(output.contains("detect"));
        assert!(output.contains("grep"));
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
