use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::{Error, Result, error_fn};

pub fn jsonrpc<I, P>(id: Option<I>, method: &str, params: &P) -> Result<Value>
where
    I: Serialize,
    P: Serialize,
{
    let mut message = Map::from_iter([
        ("jsonrpc".to_string(), Value::String("2.0".to_string())),
        ("method".to_string(), Value::String(method.to_string())),
    ]);
    if let Some(id) = id {
        message.insert(
            "id".to_string(),
            serde_json::to_value(id).map_err(error_fn!(
                Error::lsp,
                "failed to encode JSON-RPC id for {}",
                method
            ))?,
        );
    }
    let params = serde_json::to_value(params).map_err(error_fn!(
        Error::lsp,
        "failed to encode JSON-RPC params for {}",
        method
    ))?;
    // JSON-RPC 2.0 allows omitting `params` but not sending it as `null`; some servers (e.g.
    // roslyn-language-server) reject a literal null, so drop the member instead of sending it.
    if !params.is_null() {
        message.insert("params".to_string(), params);
    }
    Ok(Value::Object(message))
}
