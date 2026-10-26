use super::*;

#[test]
fn utf16_columns_resolve_expected_byte_offsets() {
    for (text, units, bytes) in [
        ("", 0, 0),
        ("abc", 2, 2),
        ("яx", 1, 2),
        ("a😀b", 1, 1),
        ("a😀b", 3, 5),
        ("a😀b", 4, 6),
        ("e\u{301}", 2, 3),
    ] {
        let line = SourceLine::new(text);
        let column = Utf16Column::new(units);
        let offset = ByteOffset::from_byte_index(text, bytes).expect("expected byte boundary");
        assert_eq!(line.byte_offset(column), Some(offset));
        assert_eq!(line.utf16_column(offset), Some(column));
    }
}

#[test]
fn source_coordinates_round_trip_on_character_boundaries() {
    for text in ["", "abc", "привет", "a😀b", "e\u{301}"] {
        let line = SourceLine::new(text);
        for index in text
            .char_indices()
            .map(|(index, _)| index)
            .chain([text.len()])
        {
            let offset = ByteOffset::from_byte_index(text, index).expect("character boundary");
            let column = line.utf16_column(offset).expect("UTF-16 column");
            assert_eq!(line.byte_offset(column), Some(offset));
        }
        assert_eq!(line.slice(ByteOffset::ZERO..line.end()), Some(text));
        assert_eq!(line.byte_offset(Utf16Column::new(999)), None);
        assert_eq!(
            line.clamped_byte_offset(Utf16Column::new(999)),
            Some(line.end())
        );
    }
}

#[test]
fn rejects_invalid_boundaries_and_reversed_slices() {
    let text = "a😀b";
    let line = SourceLine::new(text);
    assert_eq!(line.byte_offset(Utf16Column::new(2)), None);
    assert_eq!(line.clamped_byte_offset(Utf16Column::new(2)), None);
    assert_eq!(ByteOffset::from_byte_index(text, 2), None);
    assert_eq!(ByteOffset::from_byte_index(text, 99), None);
    assert_eq!(line.slice(line.end()..ByteOffset::ZERO), None);
    let foreign = SourceLine::new("ab").end();
    assert_eq!(line.utf16_column(foreign), None);
    assert_eq!(line.slice(ByteOffset::ZERO..foreign), None);
}

#[test]
fn protocol_positions_round_trip_and_render_without_overflow() {
    for (line, column) in [(0, 0), (1, 3), (u32::MAX, u32::MAX)] {
        let raw = lsp_types::Position::new(line, column);
        let position = Utf16Position::from_lsp(raw);
        assert_eq!(position.to_lsp(), raw);
        assert_eq!(position.line().one_based(), u64::from(line) + 1);
        assert_eq!(position.column().one_based(), u64::from(column) + 1);
    }
}
