use super::{render_diagnostic_paths_text, render_diagnostics_json, render_diagnostics_text};
use crate::lsp::{DiagnosticMatch, Utf16Range};
use crate::suggest::SuggestedLanguage;
use crate::test_support::{TestDir, utf16_position};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[test]
fn renders_diagnostics_text_output() {
    let dir = source_directory();
    assert_eq!(
        render_diagnostics_text(
            &[
                diagnostic(
                    "src/main.rs",
                    3,
                    5,
                    "error",
                    Some("parse-error"),
                    "ожидалась ; 😀"
                ),
                diagnostic("src/lib.rs", 9, 1, "warning", None, "unused import"),
            ],
            dir.path()
        )
        .expect("source coordinates"),
        "src/main.rs:3:5: error[parse-error]: ожидалась ; 😀\nsrc/lib.rs:9:1: warning: unused import"
    );
}

#[test]
fn sorts_diagnostics_by_position_then_severity_and_message() {
    let dir = source_directory();
    let mut diagnostics = [
        diagnostic("same.rs", 2, 1, "error", None, "later line"),
        diagnostic("same.rs", 1, 2, "error", None, "later column"),
        diagnostic("same.rs", 1, 1, "warning", None, "warning"),
        diagnostic("same.rs", 1, 1, "error", None, "я"),
        diagnostic("same.rs", 1, 1, "error", None, "a"),
    ];
    diagnostics.sort_by(DiagnosticMatch::cmp_for_output);
    assert_eq!(
        render_diagnostics_text(&diagnostics, dir.path()).expect("source coordinates"),
        "same.rs:1:1: error: a\nsame.rs:1:1: error: я\nsame.rs:1:1: warning: warning\nsame.rs:1:2: error: later column\nsame.rs:2:1: error: later line",
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
    let dir = source_directory();
    let mut item = diagnostic(
        "src/main.rs",
        3,
        5,
        "error",
        Some("parse-error"),
        "ожидалась ; 😀",
    );
    item.range = Utf16Range::new(utf16_position(2, 4), utf16_position(3, 9));
    let rendered = render_diagnostics_json(
        Path::new("workspace"),
        &BTreeSet::from(["rust".to_string()]),
        &SuggestedLanguage {
            config_id: "rust-analyzer".to_string(),
            languages: vec!["rust".to_string()],
            server: "rust-analyzer".to_string(),
            command: vec!["rust-analyzer".to_string()],
            workspace_root: dir.path().to_path_buf(),
            wait_for_index: false,
            build_index_completion: crate::config::BuildIndexCompletion::Confirmed,
            mason_extra_packages: Vec::new(),
        },
        &[item],
    )
    .expect("source coordinates");

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&rendered).expect("json should parse"),
        json!({
            "directory": "workspace",
            "detected": ["rust"],
            "server": {
                "name": "rust-analyzer",
                "languages": ["rust"],
                "command": ["rust-analyzer"],
                "workspace_root": dir.path()
            },
            "diagnostics": [{
                "path": "src/main.rs",
                "line": 3,
                "col": 5,
                "end_line": 4,
                "end_col": 10,
                "severity": "error",
                "code": "parse-error",
                "source": null,
                "message": "ожидалась ; 😀"
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
        range: Utf16Range::new(
            utf16_position(line - 1, col - 1),
            utf16_position(line - 1, col),
        ),
        severity: severity.to_string(),
        code: code.map(str::to_string),
        source: None,
        message: message.to_string(),
    }
}

fn source_directory() -> TestDir {
    let dir = TestDir::new("diagnostic-output");
    for path in ["src/main.rs", "src/lib.rs", "same.rs"] {
        dir.write_file(path, "0123456789\n".repeat(10));
    }
    dir
}

#[test]
fn diagnostic_coordinates_convert_both_endpoints_and_reject_unavailable_sources() {
    let dir = source_directory();
    dir.write_file("same.rs", "é😀\ttarget\n");
    let mut item = diagnostic("same.rs", 1, 5, "error", None, "unicode");
    item.range = Utf16Range::new(utf16_position(0, 4), utf16_position(0, 10));
    assert_eq!(
        render_diagnostics_text(std::slice::from_ref(&item), dir.path()).expect("text"),
        "same.rs:1:4: error: unicode"
    );
    let mut server = crate::test_support::suggested_language("fake", "fake", "fake", "rust");
    server.workspace_root = dir.path().to_path_buf();
    let rendered = render_diagnostics_json(
        dir.path(),
        &BTreeSet::new(),
        &server,
        std::slice::from_ref(&item),
    )
    .expect("JSON");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("JSON");
    assert_eq!(value["diagnostics"][0]["col"], 4);
    assert_eq!(value["diagnostics"][0]["end_col"], 10);
    for position in [utf16_position(0, 2), utf16_position(99, 0)] {
        item.range = Utf16Range::new(position, position);
        render_diagnostics_text(std::slice::from_ref(&item), dir.path())
            .expect_err("invalid coordinates");
    }
    item.path = "missing.rs".into();
    render_diagnostics_json(
        dir.path(),
        &BTreeSet::new(),
        &server,
        std::slice::from_ref(&item),
    )
    .expect_err("missing source");
    assert_eq!(render_diagnostic_paths_text(&[item]), "missing.rs");
}
