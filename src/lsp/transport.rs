use std::io::{BufRead, BufReader, Read, Write};

use serde_json::Value;

use crate::error::{Error, Result};

pub(crate) fn read_message<R>(reader: &mut BufReader<R>) -> Result<Option<Value>>
where
    R: Read,
{
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| Error::lsp(format!("failed to read LSP header: {error}")))?;

        if bytes == 0 {
            return if content_length.is_none() {
                Ok(None)
            } else {
                Err(Error::lsp("unexpected EOF while reading LSP headers"))
            };
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        let Some((name, value)) = trimmed.split_once(':') else {
            return Err(Error::lsp(format!("invalid LSP header: {trimmed}")));
        };

        if name.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| Error::lsp(format!("invalid Content-Length {value:?}: {error}")))?,
            );
        }
    }

    let Some(content_length) = content_length else {
        return Err(Error::lsp("missing Content-Length header"));
    };

    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|error| Error::lsp(format!("failed to read LSP body: {error}")))?;
    serde_json::from_slice(&body).map_err(|error| Error::lsp(format!("invalid JSON-RPC payload: {error}")))
}

pub(crate) fn write_message<W>(writer: &mut W, message: &Value) -> Result<()>
where
    W: Write,
{
    let body = serde_json::to_vec(message)
        .map_err(|error| Error::lsp(format!("failed to serialize JSON-RPC message: {error}")))?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .and_then(|()| writer.write_all(&body))
        .and_then(|()| writer.flush())
        .map_err(|error| Error::lsp(format!("failed to write JSON-RPC message: {error}")))
}

pub(crate) fn serialize_debug_message(message: &Value) -> String {
    serde_json::to_string_pretty(message)
        .unwrap_or_else(|_| "<failed to serialize debug message>".to_string())
}

pub(crate) fn log_debug_message(debug: bool, prefix: &str, message: &Value) {
    if debug {
        eprintln!("{prefix}{}", serialize_debug_message(message));
    }
}

#[cfg(test)]
mod tests {
    use super::{read_message, serialize_debug_message, write_message};
    use serde_json::json;
    use std::io::BufReader;

    #[test]
    fn writes_and_reads_lsp_message() {
        let mut buffer = Vec::new();
        let message = json!({"jsonrpc": "2.0", "id": 1, "result": null});
        write_message(&mut buffer, &message).expect("message should be written");

        let mut reader = BufReader::new(buffer.as_slice());
        assert_eq!(
            read_message(&mut reader).expect("message should read"),
            Some(message)
        );
    }

    #[test]
    fn serializes_debug_messages_as_json() {
        assert_eq!(
            serialize_debug_message(&json!({"jsonrpc": "2.0", "id": 1})),
            "{\n  \"id\": 1,\n  \"jsonrpc\": \"2.0\"\n}"
        );
    }
}
