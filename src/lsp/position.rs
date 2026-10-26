// LSP positions use UTF-16 because it is the only encoding this client advertises.
// Reject columns inside surrogate pairs rather than producing an invalid UTF-8 boundary.
pub(crate) fn utf16_column_to_byte(line: &str, utf16_col: usize) -> Option<usize> {
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
