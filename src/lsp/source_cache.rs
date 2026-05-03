use lsp_types::Range;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct SourceCache {
    lines: HashMap<PathBuf, Vec<String>>,
}

impl SourceCache {
    pub fn line_content(&mut self, path: &Path, line_index: usize) -> String {
        self.lines(path)
            .get(line_index)
            .cloned()
            .unwrap_or_else(|| "<line unavailable>".to_string())
    }

    pub fn range_content(&mut self, path: &Path, range: &Range) -> String {
        let lines = self.lines(path);
        let Some(start_line) = usize::try_from(range.start.line).ok() else {
            return "<line unavailable>".to_string();
        };
        let Some(end_line) = usize::try_from(range.end.line).ok() else {
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
            let start_char = if line_index == start_line {
                usize::try_from(range.start.character).ok()
            } else {
                Some(0)
            };
            let end_char = if line_index == end_line {
                usize::try_from(range.end.character).ok()
            } else {
                Some(line.chars().count())
            };
            let Some(start_byte) =
                start_char.and_then(|value| byte_index_for_character(line, value))
            else {
                return "<line unavailable>".to_string();
            };
            let Some(end_byte) = end_char.and_then(|value| byte_index_for_character(line, value))
            else {
                return "<line unavailable>".to_string();
            };
            if start_byte > end_byte {
                return "<line unavailable>".to_string();
            }
            chunks.push(line[start_byte..end_byte].to_string());
        }

        chunks.join("\n")
    }

    fn lines(&mut self, path: &Path) -> &Vec<String> {
        self.lines.entry(path.to_path_buf()).or_insert_with(|| {
            fs::read_to_string(path)
                .map(|contents| contents.lines().map(ToString::to_string).collect())
                .unwrap_or_default()
        })
    }
}

fn byte_index_for_character(line: &str, character: usize) -> Option<usize> {
    if character == line.chars().count() {
        return Some(line.len());
    }

    line.char_indices().nth(character).map(|(index, _)| index)
}
