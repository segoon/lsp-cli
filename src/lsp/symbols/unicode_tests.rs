use super::*;

#[test]
fn flat_symbol_anchors_use_utf16_columns() {
    for (source, start_line, start, end_line, end, expected_line, expected_col) in [
        ("/* привет */ class Repository {};", 0, 0, 0, 32, 1, 20),
        ("class Repository { /* привет */ };", 0, 0, 0, 33, 1, 7),
        ("/* 😀 */ class Repository {};", 0, 9, 0, 28, 1, 16),
        (
            "// привет 😀\n/* я */ class Repository {};",
            0,
            0,
            1,
            27,
            2,
            15,
        ),
        ("/* привет */ class Repository {};", 0, 0, 0, 999, 1, 20),
    ] {
        let mut fixture = source_fixture(source);
        let matches = function_matches_from_document_response(
            &fixture.flat_symbol_response(
                "Repository",
                Range::new(
                    Position::new(start_line, start),
                    Position::new(end_line, end),
                ),
            ),
            &fixture.file,
            &mut fixture.cache,
        )
        .expect("Unicode document symbols should parse");
        assert_eq!(
            (matches[0].line, matches[0].col),
            (expected_line, expected_col),
            "{source}"
        );
    }
}

#[test]
fn invalid_unicode_ranges_do_not_panic_or_find_outside_the_range() {
    let lines = vec!["😀 Repository".to_string()];
    for (start, end) in [(1, 13), (0, 1), (3, 8), (20, 30), (9, 3)] {
        assert_eq!(
            super::super::name_offset_in_range(
                &lines,
                &Range::new(Position::new(0, start), Position::new(0, end)),
                "Repository",
            ),
            None,
        );
    }
}

#[test]
fn full_content_ranges_use_utf16_columns() {
    let source = "😀 префикс Repository\nхвост 😀 конец";
    let mut fixture = source_fixture(source);
    for (start, end, expected) in [
        (Position::new(0, 11), Position::new(0, 21), "Repository"),
        (
            Position::new(0, 11),
            Position::new(1, 8),
            "Repository\nхвост 😀",
        ),
        (
            Position::new(0, 1),
            Position::new(0, 21),
            "<line unavailable>",
        ),
    ] {
        assert_eq!(
            fixture
                .cache
                .range_content(&fixture.file, &Range::new(start, end)),
            expected
        );
    }
}
