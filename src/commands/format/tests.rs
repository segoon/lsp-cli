use super::{apply_text_edits, ensure_regular_file};
use crate::test_support::TestDir;
use lsp_types::{Position, Range, TextEdit};
use std::path::Path;

fn edit(line: u32, start: u32, end: u32, text: &str) -> TextEdit {
    TextEdit {
        range: Range::new(Position::new(line, start), Position::new(line, end)),
        new_text: text.to_string(),
    }
}

fn formatted(source: &str, edits: Vec<TextEdit>) -> crate::error::Result<String> {
    apply_text_edits(source, edits, Path::new("src/main.rs"))
}

#[test]
fn applies_single_text_edit() {
    assert_eq!(
        formatted("fn main(){\n}\n", vec![edit(0, 9, 9, " ")]).expect("edits should apply"),
        "fn main() {\n}\n"
    );
}

#[test]
fn applies_multiple_text_edits_in_reverse_offset_order() {
    assert_eq!(
        formatted(
            "abc\ndef\n",
            vec![edit(1, 0, 3, "xyz"), edit(0, 0, 3, "ABC")]
        )
        .expect("edits should apply"),
        "ABC\nxyz\n"
    );
}

#[test]
fn rejects_overlapping_text_edits() {
    let error = formatted("abcdef\n", vec![edit(0, 1, 4, "X"), edit(0, 3, 5, "Y")])
        .expect_err("overlapping edits should fail");
    assert!(error.contains("overlapping edits"));
}

#[test]
fn unicode_edits_preserve_text_and_line_endings() {
    for newline in ["\n", "\r\n"] {
        let source = format!("привет 😀 old{newline}e\u{301} old{newline}");
        assert_eq!(
            formatted(&source, vec![edit(0, 10, 13, "новый"), edit(1, 3, 6, "😀")])
                .expect("UTF-16 edit ranges should apply"),
            format!("привет 😀 новый{newline}e\u{301} 😀{newline}"),
        );
    }
}

#[test]
fn invalid_unicode_edit_positions_return_errors() {
    for (line, start, end) in [(0, 1, 2), (0, 0, 1), (0, 4, 4), (1, 0, 0), (0, 3, 2)] {
        let error = formatted("😀x", vec![edit(line, start, end, "replacement")])
            .expect_err("invalid edit must fail without panicking");
        assert!(error.contains("textDocument/formatting"));
    }
}

#[test]
fn rejects_directory_for_format_file() {
    let dir = TestDir::new("format");
    let directory = dir.path().join("workspace");
    std::fs::create_dir_all(&directory).expect("workspace directory should be created");
    let error = ensure_regular_file(&directory).expect_err("directory should be rejected");
    assert!(error.contains("expected a regular file path"));
}
