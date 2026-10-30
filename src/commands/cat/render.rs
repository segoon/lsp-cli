use crate::error::{Error, Result};
use crate::lsp::{SourceLine, Utf16Column};
use lsp_types::{SemanticTokens, SemanticTokensLegend};
use serde_json::Value;

pub(super) fn highlight(
    source: &str,
    response: &Value,
    legend: &SemanticTokensLegend,
) -> Result<String> {
    let tokens: Option<SemanticTokens> = serde_json::from_value(response.clone())
        .map_err(|error| Error::lsp(format!("server returned invalid semantic tokens: {error}")))?;
    let Some(tokens) = tokens else {
        return Ok(source.to_string());
    };
    let lines = SourceLines::new(source);
    let mut output = String::new();
    let mut previous_end = 0;
    let mut line = 0_u32;
    let mut column = 0_u32;
    for token in tokens.data {
        line = line
            .checked_add(token.delta_line)
            .ok_or_else(invalid_span)?;
        column = if token.delta_line == 0 {
            column
                .checked_add(token.delta_start)
                .ok_or_else(invalid_span)?
        } else {
            token.delta_start
        };
        let end_column = column.checked_add(token.length).ok_or_else(invalid_span)?;
        let start = lines.offset(line, column).ok_or_else(invalid_span)?;
        let end = lines.offset(line, end_column).ok_or_else(invalid_span)?;
        if token.length == 0 || start < previous_end {
            return Err(invalid_span());
        }
        let token_type = usize::try_from(token.token_type)
            .ok()
            .and_then(|index| legend.token_types.get(index))
            .ok_or_else(|| {
                Error::lsp("server returned a semantic token type missing from its legend")
            })?;
        output.push_str(source.get(previous_end..start).ok_or_else(invalid_span)?);
        let text = source.get(start..end).ok_or_else(invalid_span)?;
        if let Some(style) = palette(token_type.as_str()) {
            output.push_str(style);
            output.push_str(text);
            output.push_str("\x1b[0m");
        } else {
            output.push_str(text);
        }
        previous_end = end;
    }
    output.push_str(source.get(previous_end..).ok_or_else(invalid_span)?);
    Ok(output)
}

fn invalid_span() -> Error {
    Error::lsp(
        "server returned overlapping semantic tokens or a token outside valid source character boundaries",
    )
}

struct SourceLines<'a> {
    lines: Vec<(usize, &'a str)>,
}

impl<'a> SourceLines<'a> {
    fn new(source: &'a str) -> Self {
        let mut offset = 0;
        let lines = source
            .split('\n')
            .map(|line| {
                let start = offset;
                offset += line.len() + 1;
                (start, line.strip_suffix('\r').unwrap_or(line))
            })
            .collect();
        Self { lines }
    }

    fn offset(&self, line: u32, column: u32) -> Option<usize> {
        let (start, text) = self.lines.get(usize::try_from(line).ok()?)?;
        let byte = SourceLine::new(text).byte_offset(Utf16Column::new(column))?;
        start.checked_add(byte.as_usize())
    }
}

// Standard terminal colors follow the user's terminal palette; token names remain language-neutral.
fn palette(token_type: &str) -> Option<&'static str> {
    match token_type {
        "namespace" | "type" | "class" | "enum" | "interface" | "struct" | "typeParameter" => {
            Some("\x1b[36m")
        }
        "function" | "method" | "macro" => Some("\x1b[33m"),
        "keyword" | "modifier" | "decorator" | "operator" => Some("\x1b[35m"),
        "string" | "regexp" => Some("\x1b[32m"),
        "number" | "enumMember" => Some("\x1b[34m"),
        "comment" => Some("\x1b[90m"),
        "variable" | "parameter" | "property" | "event" => Some("\x1b[37m"),
        _ => None,
    }
}
