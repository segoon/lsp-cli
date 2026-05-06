use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result, error_fn};
use crate::lsp::file_uri_to_path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticMatch {
    pub path: PathBuf,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub severity: String,
    pub code: Option<String>,
    pub source: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct PublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    range: DiagnosticRange,
    severity: Option<u32>,
    code: Option<Value>,
    source: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct DiagnosticRange {
    start: DiagnosticPosition,
    end: DiagnosticPosition,
}

#[derive(Debug, Deserialize)]
struct DiagnosticPosition {
    line: u32,
    character: u32,
}

#[derive(Debug, Deserialize)]
struct DocumentDiagnosticReport {
    kind: String,
    items: Option<Vec<Diagnostic>>,
}

fn diagnostic_params_from_notification(value: &Value) -> Result<PublishDiagnosticsParams> {
    let Some(params) = value.get("params").cloned() else {
        return Err(Error::lsp(
            "publishDiagnostics notification is missing params",
        ));
    };
    serde_json::from_value(params).map_err(error_fn!(
        Error::lsp,
        "failed to decode publishDiagnostics params"
    ))
}

fn diagnostic_matches_from_params(
    params: &PublishDiagnosticsParams,
    workspace_root: &Path,
) -> Result<Vec<DiagnosticMatch>> {
    let path = file_uri_to_path(&params.uri)?;
    let path = path
        .strip_prefix(workspace_root)
        .map_or(path.clone(), Path::to_path_buf);

    Ok(params
        .diagnostics
        .iter()
        .map(|diagnostic| DiagnosticMatch {
            path: path.clone(),
            line: diagnostic.range.start.line + 1,
            col: diagnostic.range.start.character + 1,
            end_line: diagnostic.range.end.line + 1,
            end_col: diagnostic.range.end.character + 1,
            severity: render_severity(diagnostic.severity),
            code: render_code(diagnostic.code.as_ref()),
            source: diagnostic.source.clone(),
            message: diagnostic.message.clone(),
        })
        .collect())
}

pub fn diagnostic_matches_from_notification(
    value: &Value,
    workspace_root: &Path,
) -> Result<Vec<DiagnosticMatch>> {
    let params = diagnostic_params_from_notification(value)?;
    diagnostic_matches_from_params(&params, workspace_root)
}

pub fn diagnostic_matches_from_document_response(
    value: &Value,
    document_path: &Path,
    workspace_root: &Path,
) -> Result<Vec<DiagnosticMatch>> {
    let report: DocumentDiagnosticReport =
        serde_json::from_value(value.clone()).map_err(error_fn!(
            Error::lsp,
            "failed to decode textDocument/diagnostic response"
        ))?;

    if report.kind == "unchanged" {
        return Ok(Vec::new());
    }
    if report.kind != "full" {
        return Err(Error::lsp(format!(
            "textDocument/diagnostic returned unsupported report kind {:?}",
            report.kind
        )));
    }

    let path = if let Ok(stripped) = document_path.strip_prefix(workspace_root) {
        stripped.to_path_buf()
    } else {
        document_path.to_path_buf()
    };

    Ok(report
        .items
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| DiagnosticMatch {
            path: path.clone(),
            line: diagnostic.range.start.line + 1,
            col: diagnostic.range.start.character + 1,
            end_line: diagnostic.range.end.line + 1,
            end_col: diagnostic.range.end.character + 1,
            severity: render_severity(diagnostic.severity),
            code: render_code(diagnostic.code.as_ref()),
            source: diagnostic.source,
            message: diagnostic.message,
        })
        .collect())
}

fn render_code(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        Some(_) | None => None,
    }
}

fn render_severity(value: Option<u32>) -> String {
    match value {
        Some(1) => "error",
        Some(2) => "warning",
        Some(3) => "information",
        Some(4) => "hint",
        Some(_) | None => "unknown",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{diagnostic_matches_from_document_response, diagnostic_matches_from_notification};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn decodes_publish_diagnostics_notification() {
        let diagnostics = diagnostic_matches_from_notification(
            &json!({
                "method": "textDocument/publishDiagnostics",
                "params": {
                    "uri": "file:///workspace/src/main.rs",
                    "diagnostics": [{
                        "range": {
                            "start": {"line": 1, "character": 2},
                            "end": {"line": 1, "character": 4}
                        },
                        "severity": 2,
                        "code": "unused-import",
                        "source": "rust-analyzer",
                        "message": "unused import"
                    }]
                }
            }),
            Path::new("/workspace"),
        )
        .expect("diagnostics should convert");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].path, Path::new("src/main.rs"));
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].col, 3);
        assert_eq!(diagnostics[0].severity, "warning");
        assert_eq!(diagnostics[0].code.as_deref(), Some("unused-import"));
    }

    #[test]
    fn keeps_absolute_path_when_outside_workspace() {
        let diagnostics = diagnostic_matches_from_notification(
            &json!({
                "method": "textDocument/publishDiagnostics",
                "params": {
                    "uri": "file:///tmp/external.rs",
                    "diagnostics": [{
                        "range": {
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 0, "character": 1}
                        },
                        "message": "oops"
                    }]
                }
            }),
            Path::new("/workspace"),
        )
        .expect("diagnostics should convert");

        assert_eq!(diagnostics[0].path, Path::new("/tmp/external.rs"));
        assert_eq!(diagnostics[0].severity, "unknown");
    }

    #[test]
    fn decodes_pull_diagnostics_response() {
        let diagnostics = diagnostic_matches_from_document_response(
            &json!({
                "kind": "full",
                "items": [{
                    "range": {
                        "start": {"line": 2, "character": 4},
                        "end": {"line": 2, "character": 7}
                    },
                    "severity": 1,
                    "code": "parse-error",
                    "source": "clangd",
                    "message": "expected ';'"
                }]
            }),
            Path::new("/workspace/src/main.cpp"),
            Path::new("/workspace"),
        )
        .expect("diagnostics should decode");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].path, Path::new("src/main.cpp"));
        assert_eq!(diagnostics[0].severity, "error");
        assert_eq!(diagnostics[0].code.as_deref(), Some("parse-error"));
    }

    #[test]
    fn accepts_unchanged_pull_diagnostics_response() {
        let diagnostics = diagnostic_matches_from_document_response(
            &json!({"kind": "unchanged", "resultId": "1"}),
            Path::new("/workspace/src/main.cpp"),
            Path::new("/workspace"),
        )
        .expect("unchanged report should decode");

        assert!(diagnostics.is_empty());
    }
}
