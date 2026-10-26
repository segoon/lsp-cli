use std::io::{BufRead, BufReader, Read, Write};

use serde_json::Value;

use crate::error::{Error, Result, error_fn};

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
            .map_err(error_fn!(Error::lsp, "failed to read LSP header"))?;

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
            content_length = Some(value.trim().parse::<usize>().map_err(error_fn!(
                Error::lsp,
                "invalid Content-Length {:?}",
                value
            ))?);
        }
    }

    let Some(content_length) = content_length else {
        return Err(Error::lsp("missing Content-Length header"));
    };

    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .map_err(error_fn!(Error::lsp, "failed to read LSP body"))?;
    // Parse a received value before wrapping it: Option<Value> would interpret a framed
    // JSON null as EOF and stop reading the server's subsequent response.
    serde_json::from_slice::<Value>(&body)
        .map(Some)
        .map_err(error_fn!(Error::lsp, "invalid JSON-RPC payload"))
}

pub(crate) fn write_message<W>(writer: &mut W, message: &Value) -> Result<()>
where
    W: Write,
{
    let frame = frame_message(message)?;
    writer
        .write_all(&frame)
        .and_then(|()| writer.flush())
        .map_err(error_fn!(Error::lsp, "failed to write JSON-RPC message"))
}

pub(crate) fn frame_message(message: &Value) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(message).map_err(error_fn!(
        Error::lsp,
        "failed to serialize JSON-RPC message"
    ))?;
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(&body);
    Ok(frame)
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
    fn reads_null_payload_and_following_response_before_eof() {
        // OmniSharp can send a framed null before its shutdown response. Only an empty
        // stream is EOF; deserializing into Option<Value> would conflate null with EOF.
        let messages = [
            json!(null),
            json!({"jsonrpc": "2.0", "id": 1, "result": null}),
        ];
        let mut buffer = Vec::new();
        for message in &messages {
            write_message(&mut buffer, message).expect("message should be written");
        }

        let mut reader = BufReader::new(buffer.as_slice());
        for message in messages {
            assert_eq!(
                read_message(&mut reader).expect("message should read"),
                Some(message)
            );
        }
        assert_eq!(read_message(&mut reader).expect("EOF should read"), None);
    }

    #[test]
    fn serializes_debug_messages_as_json() {
        assert_eq!(
            serialize_debug_message(&json!({"jsonrpc": "2.0", "id": 1})),
            "{\n  \"id\": 1,\n  \"jsonrpc\": \"2.0\"\n}"
        );
    }
}
