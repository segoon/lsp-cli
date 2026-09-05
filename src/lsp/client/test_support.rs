use crate::lsp::transport::read_message;
use serde_json::Value;
use std::io::{BufReader, Read};

pub(super) fn read_existing_message<R: Read>(
    reader: &mut BufReader<R>,
    parse_context: &str,
    missing_context: &str,
) -> Value {
    read_message(reader)
        .expect(parse_context)
        .expect(missing_context)
}

pub(super) fn expect_method(message: &Value, expected: &str) {
    assert_eq!(
        message.get("method").and_then(Value::as_str),
        Some(expected)
    );
}

pub(super) fn expect_id(message: &Value, expected: &str) {
    assert_eq!(message.get("id").and_then(Value::as_str), Some(expected));
}
