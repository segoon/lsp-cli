use crate::error::{Error, Result};
use crate::lsp::{LineIndex, SourceLine, Utf16Position};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceLocation {
    pub path: PathBuf,
    pub line: LineIndex,
    pub column: usize,
    input: String,
}

impl FromStr for SourceLocation {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let invalid = || {
            Error::invalid_input(
                "--at expects FILE:LINE:COLUMN with positive, one-based line and character numbers",
            )
        };
        let mut parts = input.rsplitn(3, ':');
        let Some(column) = parts
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| *n > 0)
        else {
            return Err(invalid());
        };
        let Some(line) = parts
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|n| *n > 0)
        else {
            return Err(invalid());
        };
        let Some(path) = parts.next().filter(|s| !s.is_empty()) else {
            return Err(invalid());
        };
        Ok(Self {
            path: path.into(),
            line: LineIndex::new(line - 1),
            column: column - 1,
            input: input.into(),
        })
    }
}

impl SourceLocation {
    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn resolve(&self, expected_name: Option<&str>) -> Result<Utf16Position> {
        let text = std::fs::read_to_string(&self.path).map_err(|error| {
            Error::invalid_input(format!(
                "cannot read {} for --at: {error}",
                self.path.display()
            ))
        })?;
        // Keep the empty line after a final newline available as an insertion position.
        let line = text
            .split('\n')
            .nth(self.line.as_usize().unwrap_or(usize::MAX));
        let Some(line) = line else {
            return Err(Error::invalid_input(format!(
                "{} has no line {}",
                self.path.display(),
                self.line.one_based()
            )));
        };
        let line = line.strip_suffix('\r').unwrap_or(line);
        let source = SourceLine::new(line);
        let Some(offset) = source.character_offset(self.column) else {
            return Err(Error::invalid_input(format!(
                "{}:{} has no character position {}",
                self.path.display(),
                self.line.one_based(),
                self.column + 1
            )));
        };
        if let Some(name) = expected_name {
            let suffix = source.slice(offset..source.end()).unwrap_or_default();
            if !suffix.starts_with(name) {
                return Err(Error::invalid_input(format!(
                    "expected {name:?} starting at {}, but found {:?}",
                    self.input,
                    suffix
                        .chars()
                        .take(name.chars().count())
                        .collect::<String>()
                )));
            }
        }
        let Some(column) = source.utf16_column(offset) else {
            return Err(Error::invalid_input(format!(
                "character position is too large at {}",
                self.input
            )));
        };
        Ok(Utf16Position::new(self.line, column))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NavigationTarget {
    Name(String),
    Position {
        location: SourceLocation,
        expected_name: Option<String>,
    },
}

impl NavigationTarget {
    pub fn new(name: Option<String>, at: Option<SourceLocation>) -> Result<Self> {
        if name.as_deref() == Some("") {
            return Err(Error::invalid_input(
                "symbol name or expected text must not be empty",
            ));
        }
        if let Some(location) = at {
            Ok(Self::Position {
                location,
                expected_name: name,
            })
        } else if let Some(name) = name {
            Ok(Self::Name(name))
        } else {
            Err(Error::invalid_input(
                "provide a symbol name or --at FILE:LINE:COLUMN",
            ))
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Position { location, .. } => location.input(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestDir;

    #[test]
    fn parses_locations_from_right_and_rejects_invalid_coordinates() {
        let location: SourceLocation = "some:path.rs:2:3".parse().expect("colons in path");
        assert_eq!(location.path, PathBuf::from("some:path.rs"));
        assert_eq!(location.line.one_based(), 2);
        assert_eq!(location.column, 2);
        for value in [
            "",
            "file",
            "file:1",
            ":1:1",
            "file:0:1",
            "file:1:0",
            "file:-1:2",
            "file:1:x",
            "file:4294967296:1",
        ] {
            assert!(value.parse::<SourceLocation>().is_err(), "{value}");
        }
    }

    #[test]
    fn resolves_unicode_assertions_and_boundaries() {
        let dir = TestDir::new("at-positions");
        let file = dir.write_file("unicode.rs", "é😀e\u{301}\tfoobar\r\n\r\n");
        for (line, column, expected, units) in [
            (1, 1, Some("é"), 0),
            (1, 2, Some("😀"), 1),
            (1, 3, Some("e\u{301}"), 3),
            (1, 6, Some("foo"), 6),
            (1, 12, None, 12),
            (2, 1, None, 0),
            (3, 1, None, 0),
        ] {
            let location: SourceLocation = format!("{}:{line}:{column}", file.display())
                .parse()
                .expect("location");
            assert_eq!(
                location.resolve(expected).expect("valid position").to_lsp(),
                lsp_types::Position::new(line - 1, units)
            );
        }
        for (line, column, expected) in [
            (1, 6, Some("Foo")),
            (1, 7, Some("foo")),
            (1, 13, None),
            (2, 2, None),
            (4, 1, None),
        ] {
            let location: SourceLocation = format!("{}:{line}:{column}", file.display())
                .parse()
                .expect("location");
            location
                .resolve(expected)
                .expect_err("invalid coordinate or assertion");
        }
        let empty = dir.write_file("empty.rs", "");
        let location: SourceLocation = format!("{}:1:1", empty.display())
            .parse()
            .expect("location");
        assert_eq!(
            location
                .resolve(None)
                .expect("empty insertion position")
                .to_lsp(),
            lsp_types::Position::new(0, 0)
        );
        "/missing/file.rs:1:1"
            .parse::<SourceLocation>()
            .expect("location")
            .resolve(None)
            .expect_err("unavailable source");
    }
}
