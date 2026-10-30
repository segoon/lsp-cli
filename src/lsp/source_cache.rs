#![deny(clippy::string_slice)]

use super::{ByteOffset, LineIndex, SourceLine, Utf16Column, Utf16Position, Utf16Range};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct SourceCache {
    lines: HashMap<PathBuf, std::result::Result<Vec<String>, String>>,
}

impl SourceCache {
    pub fn character_column(
        &mut self,
        path: &Path,
        position: Utf16Position,
    ) -> crate::error::Result<u64> {
        let lines = self.load_lines(path).as_ref().map_err(|error| {
            crate::error::Error::invalid_input(format!(
                "cannot read {} to convert source coordinates: {error}",
                path.display()
            ))
        })?;
        let column = position
            .line()
            .as_usize()
            .and_then(|index| lines.get(index))
            .and_then(|line| SourceLine::new(line).character_column(position.column()));
        let Some(column) = column else {
            return Err(crate::error::Error::invalid_input(format!(
                "cannot convert source position in {} at line {} to a character column: the server position is invalid",
                path.display(),
                position.line().one_based()
            )));
        };
        Ok(column)
    }

    pub fn line_content(&mut self, path: &Path, line_index: LineIndex) -> String {
        let Some(line_index) = line_index.as_usize() else {
            return "<line unavailable>".to_string();
        };
        self.lines(path)
            .get(line_index)
            .cloned()
            .unwrap_or_else(|| "<line unavailable>".to_string())
    }

    pub fn range_content(&mut self, path: &Path, range: &Utf16Range) -> String {
        let lines = self.lines(path);
        let Some(start_line) = range.start().line().as_usize() else {
            return "<line unavailable>".to_string();
        };
        let Some(end_line) = range.end().line().as_usize() else {
            return "<line unavailable>".to_string();
        };
        if start_line > end_line || end_line >= lines.len() {
            return "<line unavailable>".to_string();
        }

        let mut chunks = Vec::new();
        for line_index in start_line..=end_line {
            let Some(line) = lines.get(line_index) else {
                return "<line unavailable>".to_string();
            };
            let source_line = SourceLine::new(line);
            let start_byte = if line_index == start_line {
                source_line.byte_offset(range.start().column())
            } else {
                Some(ByteOffset::ZERO)
            };
            let end_byte = if line_index == end_line {
                source_line.byte_offset(range.end().column())
            } else {
                Some(source_line.end())
            };
            let Some(chunk) = start_byte
                .zip(end_byte)
                .and_then(|(start, end)| source_line.slice(start..end))
            else {
                return "<line unavailable>".to_string();
            };
            chunks.push(chunk.to_string());
        }

        chunks.join("\n")
    }

    pub fn range_content_with_leading_comments(
        &mut self,
        path: &Path,
        range: &Utf16Range,
    ) -> String {
        let Some(start_line) = range.start().line().as_usize() else {
            return "<line unavailable>".to_string();
        };
        let lines = self.lines(path);
        if start_line >= lines.len() {
            return "<line unavailable>".to_string();
        }
        let adjusted_start_line = comment_start_line(lines, start_line);
        let mut range = *range;
        if adjusted_start_line != start_line {
            let Some(line) = LineIndex::try_from(adjusted_start_line).ok() else {
                return "<line unavailable>".to_string();
            };
            range = Utf16Range::new(Utf16Position::new(line, Utf16Column::new(0)), range.end());
        }

        self.range_content(path, &range)
    }

    fn lines(&mut self, path: &Path) -> &[String] {
        self.load_lines(path).as_deref().unwrap_or_default()
    }

    fn load_lines(&mut self, path: &Path) -> &std::result::Result<Vec<String>, String> {
        self.lines.entry(path.to_path_buf()).or_insert_with(|| {
            fs::read_to_string(path)
                .map(|contents| {
                    contents
                        .split('\n')
                        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
                        .collect()
                })
                .map_err(|error| error.to_string())
        })
    }
}

fn comment_start_line(lines: &[String], start_line: usize) -> usize {
    let mut line_index = start_line;

    while line_index > 0 {
        let previous_line = lines[line_index - 1].trim();
        if previous_line.is_empty() {
            break;
        }
        if is_line_comment(previous_line) {
            line_index -= 1;
            continue;
        }
        if previous_line.ends_with("*/") {
            line_index = block_comment_start_line(lines, line_index - 1);
            continue;
        }
        break;
    }

    line_index
}

fn is_line_comment(line: &str) -> bool {
    line.starts_with("//")
        || (line.starts_with('#') && !line.starts_with("#!["))
        || line.starts_with("--")
}

fn block_comment_start_line(lines: &[String], end_line: usize) -> usize {
    let mut line_index = end_line;

    loop {
        if lines[line_index].contains("/*") {
            return line_index;
        }
        if line_index == 0 {
            return 0;
        }
        line_index -= 1;
    }
}
