#![warn(clippy::pedantic)]

mod cli;
mod config;
mod detect;
mod lsp;
mod suggest;

use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use cli::{BuildIndexArgs, Command as CliCommand, DetectArgs, GrepArgs, parse_args};
use config::{ConfigStore, default_config_root, load_config_store};
use detect::{DetectionResult, detect_workspace};
use lsp::{InitializeResponse, LspClient};
use serde::Deserialize;
use serde_json::{Value, json};
use suggest::{SuggestedLanguage, suggestions_for};
use url::Url;

#[derive(Debug, Eq, PartialEq)]
struct GrepMatch {
    path: PathBuf,
    line: u32,
    col: u32,
    line_content: String,
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            process::exit(2);
        }
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
        CliCommand::BuildIndex(args) => run_build_index(&args, &config),
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
    let (detection, suggestions) = analyze_path(&args.directory, config)?;
    let server = select_server(&detection, &suggestions, args.lsp.as_deref())?;
    let root_uri = path_to_file_uri(&server.workspace_root)?;
    let workspace_name = lsp::workspace_name(&server.workspace_root);

    let mut client = LspClient::new(&server.command, args.debug, args.timeout)?;
    let initialize = client
        .initialize(&root_uri, &workspace_name, false)
        .map_err(|error| format!("failed to initialize {}: {error}", server.server))?;
    ensure_workspace_symbol_support(&initialize)?;

    let response = client
        .workspace_symbol(&args.pattern)
        .map_err(|error| format!("failed to query {}: {error}", server.server));
    let shutdown = client.shutdown();
    let response = response?;
    shutdown.map_err(|error| format!("failed to stop {} cleanly: {error}", server.server))?;

    let matches = grep_matches_from_response(&response)?;

    Ok(if args.json {
        render_grep_json(args, &detection.filetypes, server, &matches)
    } else {
        render_grep_text(&matches)
    })
}

fn run_build_index(args: &BuildIndexArgs, config: &ConfigStore) -> Result<String, String> {
    let (detection, suggestions) = analyze_path(&args.directory, config)?;
    let server = select_server(&detection, &suggestions, args.lsp.as_deref())?;
    let root_uri = path_to_file_uri(&server.workspace_root)?;
    let workspace_name = lsp::workspace_name(&server.workspace_root);

    let mut client = LspClient::new(&server.command, args.debug, args.timeout)?;
    client
        .initialize(&root_uri, &workspace_name, true)
        .map_err(|error| format!("failed to initialize {}: {error}", server.server))?;

    let wait = client.wait_for_server_status_quiescent();
    let shutdown = client.shutdown();
    wait.map_err(|error| format!("failed to build index with {}: {error}", server.server))?;
    shutdown.map_err(|error| format!("failed to stop {} cleanly: {error}", server.server))?;

    Ok(String::new())
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
                detection.filetypes.iter().cloned().collect::<Vec<_>>().join(", ")
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

fn render_detect_text(detected_filetypes: &BTreeSet<String>, suggestions: &[SuggestedLanguage]) -> String {
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

fn render_grep_text(matches: &[GrepMatch]) -> String {
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

fn render_grep_json(
    args: &GrepArgs,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    matches: &[GrepMatch],
) -> String {
    json!({
        "pattern": args.pattern,
        "directory": args.directory,
        "detected": detected_filetypes,
        "server": {
            "name": server.server,
            "languages": server.languages,
            "command": server.command,
            "workspace_root": server.workspace_root,
        },
        "matches": matches
            .iter()
            .map(|matched| {
                json!({
                    "path": matched.path,
                    "line": matched.line,
                    "col": matched.col,
                    "line_content": matched.line_content,
                })
            })
            .collect::<Vec<_>>(),
    })
    .to_string()
}

fn grep_matches_from_response(response: &Value) -> Result<Vec<GrepMatch>, String> {
    if response.is_null() {
        return Ok(Vec::new());
    }

    let symbols: Vec<WorkspaceSymbolItem> = serde_json::from_value(response.clone())
        .map_err(|error| format!("failed to decode workspace/symbol response: {error}"))?;
    let mut source_cache = SourceCache::default();

    symbols
        .into_iter()
        .filter_map(|symbol| symbol.into_grep_match(&mut source_cache))
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
    fn into_grep_match(self, source_cache: &mut SourceCache) -> Option<Result<GrepMatch, String>> {
        match self {
            Self::SymbolInformation(symbol) => {
                Some(symbol.location.into_grep_match(source_cache))
            }
            Self::WorkspaceSymbol(symbol) => symbol.location.into_grep_match(source_cache),
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
    fn into_grep_match(self, source_cache: &mut SourceCache) -> Option<Result<GrepMatch, String>> {
        match self {
            Self::Full(location) => Some(location.into_grep_match(source_cache)),
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
    fn into_grep_match(self, source_cache: &mut SourceCache) -> Result<GrepMatch, String> {
        let path = file_uri_to_path(&self.uri)?;
        let line = self.range.start.line + 1;
        let col = self.range.start.character + 1;
        let line_index = usize::try_from(self.range.start.line)
            .map_err(|_| format!("line index overflow for {}", path.display()))?;
        let line_content = source_cache.line_content(&path, line_index);

        Ok(GrepMatch {
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
        GrepMatch, SourceCache, grep_matches_from_response, render_detect_json,
        render_detect_quiet, render_detect_text, render_grep_text, select_server,
    };
    use crate::detect::DetectionResult;
    use crate::suggest::SuggestedLanguage;
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
        assert_eq!(render_detect_quiet(&[example_suggestion()]), "example-lsp --stdio");
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
            render_grep_text(&[GrepMatch {
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
        assert_eq!(render_grep_text(&[]), "");
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

        let matches = grep_matches_from_response(&json!([
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
            vec![GrepMatch {
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
}
