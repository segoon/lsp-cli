#![deny(clippy::string_slice)]

use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Utf16Column(u32);

impl Utf16Column {
    pub const fn new(column: u32) -> Self {
        Self(column)
    }

    pub fn one_based(self) -> u64 {
        u64::from(self.0) + 1
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LineIndex(u32);

impl LineIndex {
    pub const fn new(line: u32) -> Self {
        Self(line)
    }

    pub fn as_usize(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }

    pub fn one_based(self) -> u64 {
        u64::from(self.0) + 1
    }
}

impl TryFrom<usize> for LineIndex {
    type Error = std::num::TryFromIntError;

    fn try_from(line: usize) -> Result<Self, Self::Error> {
        u32::try_from(line).map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ByteOffset(usize);

impl ByteOffset {
    pub const ZERO: Self = Self(0);

    // Validation is relative to this text, not a promise about other strings or snapshots.
    pub fn from_byte_index(text: &str, index: usize) -> Option<Self> {
        text.is_char_boundary(index).then_some(Self(index))
    }

    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn checked_add(self, offset: Self) -> Option<Self> {
        self.0.checked_add(offset.0).map(Self)
    }

    pub fn slice_to(self, text: &str, end: Self) -> Option<&str> {
        text.get(self.0..end.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Utf16Position {
    line: LineIndex,
    column: Utf16Column,
}

impl Utf16Position {
    pub const fn new(line: LineIndex, column: Utf16Column) -> Self {
        Self { line, column }
    }

    // Only use this at protocol boundaries after the session's UTF-16 encoding is verified.
    pub fn from_lsp(position: lsp_types::Position) -> Self {
        Self::new(
            LineIndex::new(position.line),
            Utf16Column::new(position.character),
        )
    }

    pub fn to_lsp(self) -> lsp_types::Position {
        lsp_types::Position::new(self.line.0, self.column.0)
    }

    pub fn line(self) -> LineIndex {
        self.line
    }

    pub fn column(self) -> Utf16Column {
        self.column
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Utf16Range {
    start: Utf16Position,
    end: Utf16Position,
}

impl Utf16Range {
    pub const fn new(start: Utf16Position, end: Utf16Position) -> Self {
        Self { start, end }
    }

    pub fn from_lsp(range: lsp_types::Range) -> Self {
        Self::new(
            Utf16Position::from_lsp(range.start),
            Utf16Position::from_lsp(range.end),
        )
    }

    pub fn start(self) -> Utf16Position {
        self.start
    }

    pub fn end(self) -> Utf16Position {
        self.end
    }

    // Symbol fallback matching historically includes the end anchor; extraction is end-exclusive.
    pub fn contains_anchor(self, position: Utf16Position) -> bool {
        self.start <= position && position <= self.end
    }

    pub fn size_hint(self) -> u64 {
        let lines = u64::from(self.end.line.0.saturating_sub(self.start.line.0));
        let columns = u64::from(self.end.column.0.saturating_sub(self.start.column.0));
        lines * 1_000_000 + columns
    }
}

#[derive(Clone, Copy)]
pub struct SourceLine<'a> {
    text: &'a str,
}

impl<'a> SourceLine<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text }
    }

    pub fn end(self) -> ByteOffset {
        ByteOffset(self.text.len())
    }

    pub fn slice(self, range: Range<ByteOffset>) -> Option<&'a str> {
        range.start.slice_to(self.text, range.end)
    }

    pub fn utf16_column(self, offset: ByteOffset) -> Option<Utf16Column> {
        let prefix = self.text.get(..offset.0)?;
        u32::try_from(prefix.encode_utf16().count())
            .ok()
            .map(Utf16Column)
    }

    pub fn byte_offset(self, column: Utf16Column) -> Option<ByteOffset> {
        self.offset_for_units(usize::try_from(column.0).ok()?)
    }

    pub fn clamped_byte_offset(self, column: Utf16Column) -> Option<ByteOffset> {
        let units = usize::try_from(column.0).ok()?;
        self.offset_for_units(units.min(self.text.encode_utf16().count()))
    }

    // LSP columns count UTF-16 units. A surrogate-pair interior cannot delimit a Rust string.
    fn offset_for_units(self, column: usize) -> Option<ByteOffset> {
        let mut units = 0;
        for (index, ch) in self.text.char_indices() {
            if units == column {
                return Some(ByteOffset(index));
            }
            units += ch.len_utf16();
            if units > column {
                return None;
            }
        }
        (units == column).then_some(self.end())
    }
}

#[cfg(test)]
mod tests;
