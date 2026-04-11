mod config;
mod detect;
mod suggest;

use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;
use std::process;

use config::{default_config_root, load_config_store};
use detect::detect_workspace;
use suggest::{SuggestedLanguage, suggestions_for};

struct Args {
    path: PathBuf,
    json: bool,
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
            eprintln!("failed to load config from {}: {error}", config_root.display());
            process::exit(1);
        }
    };

    let detection = match detect_workspace(&args.path, &config.filetypes) {
        Ok(detection) => detection,
        Err(error) => {
            eprintln!("failed to scan {}: {error}", args.path.display());
            process::exit(1);
        }
    };

    let suggestions = match suggestions_for(&config.lsps, &detection, &args.path) {
        Ok(suggestions) => suggestions,
        Err(error) => {
            eprintln!("failed to build suggestions: {error}");
            process::exit(1);
        }
    };

    let output = if args.json {
        render_json(&suggestions)
    } else {
        render_text(&detection.filetypes, &suggestions)
    };

    println!("{output}");
}

fn parse_args<I>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = String>,
{
    let mut path = None;
    let mut json = false;

    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            flag if flag.starts_with('-') => {
                return Err(format!(
                    "unknown flag: {flag}\nusage: lsp-cli [PATH] [--json]"
                ));
            }
            _ => {
                if path.is_some() {
                    return Err("usage: lsp-cli [PATH] [--json]".to_string());
                }

                path = Some(PathBuf::from(arg));
            }
        }
    }

    Ok(Args {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        json,
    })
}

fn render_text(detected_filetypes: &BTreeSet<String>, suggestions: &[SuggestedLanguage]) -> String {
    if suggestions.is_empty() {
        return "No supported languages detected".to_string();
    }

    let detected = if detected_filetypes.is_empty() {
        "none".to_string()
    } else {
        detected_filetypes.iter().cloned().collect::<Vec<_>>().join(", ")
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

fn render_json(suggestions: &[SuggestedLanguage]) -> String {
    let languages = suggestions
        .iter()
        .map(|suggestion| {
            let command = suggestion
                .command
                .iter()
                .map(|part| format!("\"{}\"", escape_json(part)))
                .collect::<Vec<_>>()
                .join(",");

            format!(
                "{{\"name\":\"{}\",\"server\":\"{}\",\"command\":[{}]}}",
                escape_json(&suggestion.name),
                escape_json(&suggestion.server),
                command
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!("{{\"languages\":[{languages}]}}")
}

fn escape_json(input: &str) -> String {
    let mut escaped = String::new();

    for ch in input.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                use std::fmt::Write;
                let _ = write!(escaped, "\\u{:04x}", ch as u32);
            }
            ch => escaped.push(ch),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::{parse_args, render_json, render_text};
    use crate::suggest::SuggestedLanguage;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn clangd_suggestion() -> SuggestedLanguage {
        SuggestedLanguage {
            name: "clangd".to_string(),
            server: "clangd".to_string(),
            command: vec!["clangd".to_string(), "--background-index".to_string()],
        }
    }

    #[test]
    fn parses_default_arguments() {
        let args = parse_args(Vec::<String>::new()).expect("args should parse");

        assert_eq!(args.path, PathBuf::from("."));
        assert!(!args.json);
    }

    #[test]
    fn parses_json_flag_and_path() {
        let args =
            parse_args(vec!["src".to_string(), "--json".to_string()]).expect("args should parse");

        assert_eq!(args.path, PathBuf::from("src"));
        assert!(args.json);
    }

    #[test]
    fn renders_empty_text_output() {
        assert_eq!(render_text(&BTreeSet::new(), &[]), "No supported languages detected");
    }

    #[test]
    fn renders_text_output() {
        let detected = BTreeSet::from(["c".to_string(), "cpp".to_string()]);

        assert_eq!(
            render_text(&detected, &[clangd_suggestion()]),
            "Detected: c, cpp\nSuggested command: clangd --background-index"
        );
    }

    #[test]
    fn renders_text_output_without_detected_filetypes() {
        assert_eq!(
            render_text(&BTreeSet::new(), &[clangd_suggestion()]),
            "Detected: none\nSuggested command: clangd --background-index"
        );
    }

    #[test]
    fn renders_empty_json_output() {
        assert_eq!(render_json(&[]), "{\"languages\":[]}");
    }

    #[test]
    fn renders_json_output() {
        assert_eq!(
            render_json(&[clangd_suggestion()]),
            concat!(
                "{\"languages\":[",
                "{\"name\":\"clangd\",\"server\":\"clangd\",\"command\":[\"clangd\",\"--background-index\"]}",
                "]}"
            )
        );
    }
}
