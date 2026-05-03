use super::{render_diagnostic_paths_text, render_diagnostics_json, render_diagnostics_text};
use crate::lsp::DiagnosticMatch;
use crate::suggest::SuggestedLanguage;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[test]
fn renders_diagnostics_text_output() {
    assert_eq!(
        render_diagnostics_text(&[
            diagnostic(
                "src/main.rs",
                3,
                5,
                "error",
                Some("parse-error"),
                "expected ;"
            ),
            diagnostic("src/lib.rs", 9, 1, "warning", None, "unused import"),
        ]),
        "src/main.rs:3:5: error[parse-error]: expected ;\nsrc/lib.rs:9:1: warning: unused import"
    );
}

#[test]
fn renders_unique_diagnostic_paths() {
    assert_eq!(
        render_diagnostic_paths_text(&[
            diagnostic("src/main.rs", 1, 1, "error", None, "oops"),
            diagnostic("src/main.rs", 2, 1, "warning", None, "warn"),
            diagnostic("src/lib.rs", 3, 1, "warning", None, "warn"),
        ]),
        "src/main.rs\nsrc/lib.rs"
    );
}

#[test]
fn renders_diagnostics_json_output() {
    let rendered = render_diagnostics_json(
        Path::new("workspace"),
        &BTreeSet::from(["rust".to_string()]),
        &SuggestedLanguage {
            config_id: "rust-analyzer".to_string(),
            languages: vec!["rust".to_string()],
            server: "rust-analyzer".to_string(),
            command: vec!["rust-analyzer".to_string()],
            workspace_root: PathBuf::from("/workspace"),
            wait_for_index: false,
        },
        &[diagnostic(
            "src/main.rs",
            3,
            5,
            "error",
            Some("parse-error"),
            "expected ;",
        )],
    );

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
        json!({
            "directory": "workspace",
            "detected": ["rust"],
            "server": {
                "name": "rust-analyzer",
                "languages": ["rust"],
                "command": ["rust-analyzer"],
                "workspace_root": "/workspace"
            },
            "diagnostics": [{
                "path": "src/main.rs",
                "line": 3,
                "col": 5,
                "end_line": 3,
                "end_col": 6,
                "severity": "error",
                "code": "parse-error",
                "source": null,
                "message": "expected ;"
            }]
        })
    );
}

fn diagnostic(
    path: &str,
    line: u32,
    col: u32,
    severity: &str,
    code: Option<&str>,
    message: &str,
) -> DiagnosticMatch {
    DiagnosticMatch {
        path: PathBuf::from(path),
        line,
        col,
        end_line: line,
        end_col: col + 1,
        severity: severity.to_string(),
        code: code.map(str::to_string),
        source: None,
        message: message.to_string(),
    }
}
