use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::{Error, Result};

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
            serde_json::to_value(id)
                .map_err(|error| Error::lsp(format!("failed to encode JSON-RPC id for {method}: {error}")))?,
        );
    }
    message.insert(
        "params".to_string(),
        serde_json::to_value(params)
            .map_err(|error| Error::lsp(format!("failed to encode JSON-RPC params for {method}: {error}")))?,
    );
    Ok(Value::Object(message))
}
