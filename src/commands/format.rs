use crate::cli::FormatArgs;
use crate::commands::common::{connect_lsp_client, prepare_workspace};
use crate::config::ConfigStore;
use crate::lsp::{ensure_formatting_support, path_to_file_uri};
use lsp_types::{Position, TextEdit};
use serde_json::json;
use std::fs;
use std::path::Path;

#[cfg(test)]
mod tests;

pub(super) fn run(args: &FormatArgs, config: &ConfigStore) -> Result<String, String> {
    ensure_regular_file(&args.path)?;

    let workspace = prepare_workspace(
        &args.path,
        args.server.selection.lsp.as_deref(),
        args.server.selection.lang.as_deref(),
        args.server.download,
        config,
    )?;
    let mut client =
        connect_lsp_client(&workspace, args.detach, args.server.debug, args.timeout)?;
    let initialize = client
        .initialize(&workspace.root_uri, &workspace.workspace_name, false)
        .map_err(|error| format!("failed to initialize {}: {error}", workspace.server.server))?;
    ensure_formatting_support(&initialize).map_err(|error| {
        format!(
            "{} does not support format because {error}",
            workspace.server.server
        )
    })?;

    let uri = path_to_file_uri(&args.path)?;
    client.open_document(&args.path, &uri).map_err(|error| {
        format!(
            "failed to open {} with {}: {error}",
            args.path.display(),
            workspace.server.server
        )
    })?;
    let response = client.format_document(&uri).map_err(|error| {
        format!(
            "failed to format {} with {}: {error}",
            args.path.display(),
            workspace.server.server
        )
    })?;

    let original = fs::read_to_string(&args.path)
        .map_err(|error| format!("failed to read {}: {error}", args.path.display()))?;
    let formatted = apply_formatting_response(&response, &original, &args.path)?;
    let changed = formatted != original;

    if changed && !args.check && !args.stdout {
        fs::write(&args.path, formatted.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", args.path.display()))?;
    }

    client.shutdown().map_err(|error| {
        format!(
            "failed to stop {} cleanly: {error}",
            workspace.server.server
        )
    })?;

    if args.check && changed {
        return Err(format!("{} is not formatted", args.path.display()));
    }

    Ok(if args.stdout {
        formatted
    } else if args.json {
        json!({
            "file": args.path,
            "server": {
                "name": workspace.server.server,
                "languages": workspace.server.languages,
                "command": workspace.server.command,
                "workspace_root": workspace.server.workspace_root,
            },
            "changed": changed,
        })
        .to_string()
    } else {
        String::new()
    })
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "format expected a file path, but {} does not exist",
                path.display()
            )
        } else {
            format!("failed to inspect {}: {error}", path.display())
        }
    })?;

    if metadata.is_file() {
        return Ok(());
    }

    Err(format!(
        "format expected a regular file path, but {} is not a file",
        path.display()
    ))
}

fn apply_formatting_response(
    response: &serde_json::Value,
    source: &str,
    path: &Path,
) -> Result<String, String> {
    let edits: Option<Vec<TextEdit>> = serde_json::from_value(response.clone())
        .map_err(|error| format!("failed to decode textDocument/formatting response: {error}"))?;
    apply_text_edits(source, edits.unwrap_or_default(), path)
}

fn apply_text_edits(source: &str, mut edits: Vec<TextEdit>, path: &Path) -> Result<String, String> {
    if edits.is_empty() {
        return Ok(source.to_string());
    }

    let line_starts = line_start_offsets(source);
    let mut spans = edits
        .drain(..)
        .map(|edit| {
            let start = position_to_offset(source, &line_starts, edit.range.start, path)?;
            let end = position_to_offset(source, &line_starts, edit.range.end, path)?;
            if start > end {
                return Err(format!(
                    "textDocument/formatting returned an invalid edit range for {}",
                    path.display()
                ));
            }
            Ok((start, end, edit.new_text))
        })
        .collect::<Result<Vec<_>, String>>()?;

    spans.sort_by(|left, right| right.0.cmp(&left.0).then(right.1.cmp(&left.1)));

    for window in spans.windows(2) {
        let current = &window[0];
        let next = &window[1];
        if next.1 > current.0 {
            return Err(format!(
                "textDocument/formatting returned overlapping edits for {}",
                path.display()
            ));
        }
    }

    let mut formatted = source.to_string();
    for (start, end, new_text) in spans {
        formatted.replace_range(start..end, &new_text);
    }

    Ok(formatted)
}

fn line_start_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (index, ch) in source.char_indices() {
        if ch == '\n' {
            offsets.push(index + ch.len_utf8());
        }
    }
    offsets
}

fn position_to_offset(
    source: &str,
    line_starts: &[usize],
    position: Position,
    path: &Path,
) -> Result<usize, String> {
    let line = usize::try_from(position.line)
        .map_err(|_| format!("line index overflow for {}", path.display()))?;
    let Some(&line_start) = line_starts.get(line) else {
        return Err(format!(
            "textDocument/formatting returned a line outside {}",
            path.display()
        ));
    };
    let line_end = if let Some(next_start) = line_starts.get(line + 1) {
        next_start.saturating_sub(1)
    } else {
        source.len()
    };
    let line_text = &source[line_start..line_end];
    let utf16_col = usize::try_from(position.character)
        .map_err(|_| format!("column overflow for {}", path.display()))?;
    let byte_in_line = utf16_column_to_byte(line_text, utf16_col).ok_or_else(|| {
        format!(
            "textDocument/formatting returned a column outside {}",
            path.display()
        )
    })?;
    Ok(line_start + byte_in_line)
}

fn utf16_column_to_byte(line: &str, utf16_col: usize) -> Option<usize> {
    if utf16_col == 0 {
        return Some(0);
    }

    let mut units = 0;
    for (index, ch) in line.char_indices() {
        if units == utf16_col {
            return Some(index);
        }
        units += ch.len_utf16();
        if units == utf16_col {
            return Some(index + ch.len_utf8());
        }
        if units > utf16_col {
            return None;
        }
    }

    (units == utf16_col).then_some(line.len())
}
