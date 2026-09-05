use std::env;
use std::io::{self, BufRead, BufReader, Write};

use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var_os("LSP_CLI_E2E_RUN_MARKER").is_some() {
        println!("fake LSP server replaced lsp-cli");
        return Ok(());
    }

    let mut input = BufReader::new(io::stdin().lock());
    let mut output = io::stdout().lock();
    let mut root_uri = String::new();
    let mut report_status = false;

    while let Some(message) = read_message(&mut input)? {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        if method == "exit" {
            break;
        }
        if method == "initialized" {
            if report_status {
                write_message(
                    &mut output,
                    &json!({
                        "jsonrpc": "2.0",
                        "method": "experimental/serverStatus",
                        "params": {"health": "ok", "quiescent": true}
                    }),
                )?;
            }
            continue;
        }
        let Some(id) = message.get("id").cloned() else {
            continue;
        };

        let result = match method {
            "initialize" => {
                root_uri = message["params"]["rootUri"]
                    .as_str()
                    .unwrap_or("file:///")
                    .to_string();
                report_status =
                    message["params"]["capabilities"]["experimental"]["serverStatusNotification"]
                        .as_bool()
                        .unwrap_or(false);
                initialize_result()
            }
            "workspace/symbol" | "textDocument/documentSymbol" => {
                json!([symbol(&root_uri)])
            }
            "textDocument/diagnostic" => json!({
                "kind": "full",
                "items": [{
                    "range": range(1, 2, 13),
                    "severity": 2,
                    "code": "fixture",
                    "source": "e2e-fake-lsp",
                    "message": "synthetic diagnostic"
                }]
            }),
            "textDocument/formatting" => json!([{
                "range": range(1, 0, 13),
                "newText": "    formatted"
            }]),
            "textDocument/references" | "textDocument/definition" | "textDocument/declaration" => {
                json!([location(&root_uri)])
            }
            "textDocument/prepareCallHierarchy" => json!([call_item(&root_uri)]),
            "callHierarchy/incomingCalls" => {
                json!([{"from": call_item(&root_uri), "fromRanges": []}])
            }
            "callHierarchy/outgoingCalls" => {
                json!([{"to": call_item(&root_uri), "fromRanges": []}])
            }
            "shutdown" => Value::Null,
            _ => {
                write_message(
                    &mut output,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {"code": -32601, "message": format!("unsupported method {method}")}
                    }),
                )?;
                continue;
            }
        };
        write_message(
            &mut output,
            &json!({"jsonrpc": "2.0", "id": id, "result": result}),
        )?;
    }
    Ok(())
}

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "workspaceSymbolProvider": true,
            "documentSymbolProvider": true,
            "definitionProvider": true,
            "declarationProvider": true,
            "referencesProvider": true,
            "callHierarchyProvider": true,
            "documentFormattingProvider": true,
            "diagnosticProvider": {
                "identifier": "fixture",
                "interFileDependencies": false,
                "workspaceDiagnostics": false
            }
        },
        "serverInfo": {"name": "e2e-fake-lsp", "version": "1"}
    })
}

fn symbol(root_uri: &str) -> Value {
    json!({
        "name": "Target",
        "kind": 12,
        "location": location(root_uri)
    })
}

fn location(root_uri: &str) -> Value {
    json!({"uri": format!("{root_uri}/main.fake"), "range": range(0, 3, 9)})
}

fn call_item(root_uri: &str) -> Value {
    json!({
        "name": "Target",
        "kind": 12,
        "uri": format!("{root_uri}/main.fake"),
        "range": range(0, 0, 13),
        "selectionRange": range(0, 3, 9)
    })
}

fn range(line: u32, start: u32, end: u32) -> Value {
    json!({
        "start": {"line": line, "character": start},
        "end": {"line": line, "character": end}
    })
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        if header == "\r\n" {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = Some(value.trim().parse::<usize>().map_err(io::Error::other)?);
        }
    }
    let length = content_length.ok_or_else(|| io::Error::other("missing Content-Length"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(io::Error::other)
}

fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}
