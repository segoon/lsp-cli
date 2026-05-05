use crate::cli::DiagnosticsArgs;
use crate::commands::common::{connect_lsp_client, prepare_workspace};
use crate::commands::symbol_query::{render_paths_text, truncate_items};
use crate::config::ConfigStore;
use crate::detect::matching_files;
use crate::lsp::{
    DiagnosticMatch, LspClient, diagnostic_matches_from_document_response,
    diagnostic_matches_from_notification, diagnostics_supported, path_to_file_uri,
};
use crate::suggest::SuggestedLanguage;
use serde_json::json;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

const DIAGNOSTIC_QUIET_PERIOD: Duration = Duration::from_millis(250);
const DIAGNOSTIC_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) fn run(args: &DiagnosticsArgs, config: &ConfigStore) -> Result<String, String> {
    let result = run_diagnostics_query(args, config)?;
    let diagnostics = truncate_items(
        result.diagnostics,
        args.query.query.limit,
        if args.query.query.json {
            "items"
        } else {
            "lines"
        },
    );

    Ok(if args.query.query.json {
        render_diagnostics_json(
            &args.query.query.directory,
            &result.detected_filetypes,
            &result.server,
            &diagnostics,
        )
    } else if args.query.files_with_matches {
        render_diagnostic_paths_text(&diagnostics)
    } else {
        render_diagnostics_text(&diagnostics)
    })
}

struct DiagnosticsQueryResult {
    detected_filetypes: BTreeSet<String>,
    server: SuggestedLanguage,
    diagnostics: Vec<DiagnosticMatch>,
}

fn run_diagnostics_query(
    args: &DiagnosticsArgs,
    config: &ConfigStore,
) -> Result<DiagnosticsQueryResult, String> {
    let query = &args.query.query;
    let workspace = prepare_workspace(
        &query.directory,
        query.server(),
        query.language(),
        args.query.download,
        config,
    )?;
    let files = matching_files(
        &query.directory,
        &config.filetypes,
        &workspace.allowed_filetypes,
    )
    .map_err(|error| {
        format!("failed to scan {}: {error}", query.directory.display())
    })?;

    let mut client = connect_lsp_client(
        &workspace,
        args.query.detach,
        query.debug,
        query.timeout,
    )?;
    let initialize = client
        .initialize(&workspace.root_uri, &workspace.workspace_name, true)
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;

    let mut diagnostics = if diagnostics_supported(&initialize) {
        collect_pull_diagnostics(
            &mut client,
            &workspace.server.server,
            &workspace.server.workspace_root,
            &files,
        )?
    } else {
        collect_push_diagnostics(
            &mut client,
            &workspace.server.server,
            &workspace.server.workspace_root,
            &files,
            query.timeout,
        )?
    };
    diagnostics.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.line.cmp(&right.line))
            .then(left.col.cmp(&right.col))
            .then(left.severity.cmp(&right.severity))
            .then(left.message.cmp(&right.message))
    });
    client.shutdown().map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;

    Ok(DiagnosticsQueryResult {
        detected_filetypes: workspace.detection.filetypes,
        server: workspace.server,
        diagnostics,
    })
}

fn collect_pull_diagnostics(
    client: &mut LspClient,
    server_name: &str,
    workspace_root: &Path,
    files: &[PathBuf],
) -> Result<Vec<DiagnosticMatch>, String> {
    let mut diagnostics = Vec::new();

    for file in files {
        let uri = path_to_file_uri(file)?;
        client.open_document(file, &uri).map_err(|error| {
            format!(
                "failed to open {} with {}: {error}",
                file.display(),
                server_name
            )
        })?;
        let response = client.document_diagnostic(&uri).map_err(|error| {
            format!(
                "failed to query diagnostics from {} for {}: {error}",
                server_name,
                file.display()
            )
        })?;
        diagnostics.extend(diagnostic_matches_from_document_response(
            &response,
            file,
            workspace_root,
        )?);
    }

    Ok(diagnostics)
}

fn collect_push_diagnostics(
    client: &mut LspClient,
    server_name: &str,
    workspace_root: &Path,
    files: &[PathBuf],
    timeout: Duration,
) -> Result<Vec<DiagnosticMatch>, String> {
    for file in files {
        let uri = path_to_file_uri(file)?;
        client.open_document(file, &uri).map_err(|error| {
            format!(
                "failed to open {} with {}: {error}",
                file.display(),
                server_name
            )
        })?;
    }

    wait_for_push_diagnostics(client, timeout)?;
    collect_diagnostic_matches(client, workspace_root)
}

fn wait_for_push_diagnostics(client: &mut LspClient, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    let mut saw_diagnostics = false;
    let mut last_activity = started;
    let mut previous_diagnostic_count = client.published_diagnostics_len();

    while let Some(remaining) = timeout.checked_sub(started.elapsed()) {
        let poll = remaining.min(DIAGNOSTIC_POLL_INTERVAL);

        client.collect_diagnostics(poll)?;
        client.drain_server_notifications()?;

        let current_diagnostic_count = client.published_diagnostics_len();
        if current_diagnostic_count > 0 {
            saw_diagnostics = true;
        }
        if current_diagnostic_count != previous_diagnostic_count {
            last_activity = Instant::now();
        }
        previous_diagnostic_count = current_diagnostic_count;

        if saw_diagnostics && last_activity.elapsed() >= DIAGNOSTIC_QUIET_PERIOD {
            break;
        }
    }

    Ok(())
}

fn collect_diagnostic_matches(
    client: &mut LspClient,
    workspace_root: &Path,
) -> Result<Vec<DiagnosticMatch>, String> {
    let notifications = client.take_published_diagnostics();
    let mut diagnostics = Vec::new();

    for notification in notifications {
        diagnostics.extend(diagnostic_matches_from_notification(
            &notification,
            workspace_root,
        )?);
    }

    Ok(diagnostics)
}

fn render_diagnostic_paths_text(diagnostics: &[DiagnosticMatch]) -> String {
    let mut seen = HashSet::new();
    let paths = diagnostics
        .iter()
        .filter(|diagnostic| seen.insert(diagnostic.path.clone()))
        .map(|diagnostic| diagnostic.path.clone())
        .collect::<Vec<_>>();
    render_paths_text(&paths)
}

fn render_diagnostics_text(diagnostics: &[DiagnosticMatch]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let code = diagnostic
                .code
                .as_deref()
                .map(|value| format!("[{value}]"))
                .unwrap_or_default();
            format!(
                "{}:{}:{}: {}{}: {}",
                diagnostic.path.display(),
                diagnostic.line,
                diagnostic.col,
                diagnostic.severity,
                code,
                diagnostic.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_diagnostics_json(
    directory: &Path,
    detected_filetypes: &BTreeSet<String>,
    server: &SuggestedLanguage,
    diagnostics: &[DiagnosticMatch],
) -> String {
    json!({
        "directory": directory,
        "detected": detected_filetypes,
        "server": {
            "name": server.server,
            "languages": server.languages,
            "command": server.command,
            "workspace_root": server.workspace_root,
        },
        "diagnostics": diagnostics.iter().map(|diagnostic| json!({
            "path": diagnostic.path,
            "line": diagnostic.line,
            "col": diagnostic.col,
            "end_line": diagnostic.end_line,
            "end_col": diagnostic.end_col,
            "severity": diagnostic.severity,
            "code": diagnostic.code,
            "source": diagnostic.source,
            "message": diagnostic.message,
        })).collect::<Vec<_>>(),
    })
    .to_string()
}
