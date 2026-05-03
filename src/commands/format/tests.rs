use super::{apply_text_edits, ensure_regular_file, utf16_column_to_byte};
use lsp_types::{Position, Range, TextEdit};
use std::path::Path;

#[test]
fn applies_single_text_edit() {
    let formatted = apply_text_edits(
        "fn main(){\n}\n",
        vec![TextEdit {
            range: Range {
                start: Position::new(0, 9),
                end: Position::new(0, 9),
            },
            new_text: " ".to_string(),
        }],
        Path::new("src/main.rs"),
    )
    .expect("edits should apply");

    assert_eq!(formatted, "fn main() {\n}\n");
}

#[test]
fn applies_multiple_text_edits_in_reverse_offset_order() {
    let formatted = apply_text_edits(
        "abc\ndef\n",
        vec![
            TextEdit {
                range: Range {
                    start: Position::new(1, 0),
                    end: Position::new(1, 3),
                },
                new_text: "xyz".to_string(),
            },
            TextEdit {
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 3),
                },
                new_text: "ABC".to_string(),
            },
        ],
        Path::new("src/main.rs"),
    )
    .expect("edits should apply");

    assert_eq!(formatted, "ABC\nxyz\n");
}

#[test]
fn rejects_overlapping_text_edits() {
    let error = apply_text_edits(
        "abcdef\n",
        vec![
            TextEdit {
                range: Range {
                    start: Position::new(0, 1),
                    end: Position::new(0, 4),
                },
                new_text: "X".to_string(),
            },
            TextEdit {
                range: Range {
                    start: Position::new(0, 3),
                    end: Position::new(0, 5),
                },
                new_text: "Y".to_string(),
            },
        ],
        Path::new("src/main.rs"),
    )
    .expect_err("overlapping edits should fail");

    assert!(error.contains("overlapping edits"));
}

#[test]
fn converts_utf16_columns_to_bytes() {
    assert_eq!(utf16_column_to_byte("a😀b", 0), Some(0));
    assert_eq!(utf16_column_to_byte("a😀b", 1), Some(1));
    assert_eq!(utf16_column_to_byte("a😀b", 3), Some(5));
    assert_eq!(utf16_column_to_byte("a😀b", 4), Some(6));
}

#[test]
fn rejects_directory_for_format_file() {
    let error = ensure_regular_file(Path::new("/tmp"))
        .expect_err("directory should be rejected as format target");

    assert!(error.contains("expected a regular file path"));
}
