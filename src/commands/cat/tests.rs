use super::{color_enabled, render::highlight, run_with_color};
use crate::cli::CatArgs;
use crate::config::{CliConfig, ConfigStore};
use crate::test_support::TestDir;
use lsp_types::SemanticTokensLegend;
use serde_json::{Value, json};
use std::ffi::OsStr;

fn highlighted(source: &str, types: &[&str], data: &Value) -> crate::error::Result<String> {
    let legend = SemanticTokensLegend {
        token_types: types
            .iter()
            .map(|name| (*name).to_string().into())
            .collect(),
        token_modifiers: Vec::new(),
    };
    highlight(source, data, &legend)
}

#[test]
fn colors_relative_tokens_using_server_legend_and_preserves_unicode_and_crlf() {
    assert_eq!(
        highlighted(
            "😀 fn привет\r\n42",
            &["number", "keyword", "function"],
            &json!({"data":[0,3,2,1,0, 0,3,6,2,0, 1,0,2,0,0]})
        )
        .expect("highlight"),
        "😀 \x1b[35mfn\x1b[0m \x1b[33mпривет\x1b[0m\r\n\x1b[34m42\x1b[0m"
    );
}

#[test]
fn preserves_uncovered_text_unknown_types_and_empty_responses() {
    for source in ["", "abc", "abc\n", "abc\r\n"] {
        for response in [Value::Null, json!({"data":[]})] {
            assert_eq!(
                highlighted(source, &[], &response).expect("plain text"),
                source
            );
        }
    }
    assert_eq!(
        highlighted("abc", &["customType"], &json!({"data":[0,0,3,0,0]})).expect("custom type"),
        "abc"
    );
}

#[test]
fn rejects_invalid_tokens_without_panicking() {
    for data in [
        json!([0, 0, 1]),                      // Incomplete token encoding.
        json!([0, 1, 1, 0, 0]),                // Inside an emoji's surrogate pair.
        json!([0, 0, 1, 0, 0]),                // End inside a surrogate pair.
        json!([1, 0, 1, 0, 0]),                // Outside the document.
        json!([0, 0, 4, 0, 0]),                // Beyond the line.
        json!([0, 0, 2, 1, 0]),                // Missing legend entry.
        json!([0, 0, 2, 0, 0, 0, 1, 2, 0, 0]), // Overlap.
        json!([0, 0, 0, 0, 0]),                // Empty token.
        json!([0, 2, u32::MAX, 0, 0]),         // Column overflow.
    ] {
        highlighted("😀x", &["string"], &json!({"data":data})).expect_err("invalid tokens");
    }
}

#[test]
fn color_policy_honors_terminal_no_color_and_dumb_term() {
    for (terminal, no_color, term, expected) in [
        (true, None, None, true),
        (false, None, None, false),
        (true, Some("1"), None, false),
        (true, Some(""), None, true),
        (true, None, Some("dumb"), false),
        (true, None, Some("xterm-256color"), true),
    ] {
        assert_eq!(
            color_enabled(terminal, no_color.map(OsStr::new), term.map(OsStr::new)),
            expected
        );
    }
}

#[test]
fn plain_cat_needs_no_language_configuration_and_rejects_non_files() {
    let dir = TestDir::new("cat");
    let config = ConfigStore {
        filetypes: vec![],
        lsps: vec![],
        cli: CliConfig::default(),
    };
    let mut args = CatArgs {
        path: dir.path().join("source.unknown"),
        server: crate::cli::InstallDebugArgs {
            selection: crate::cli::SelectionArgs {
                lang: None,
                lsp: None,
            },
            download: false,
            debug: false,
        },
        detach: false,
        timeout: std::time::Duration::from_secs(1),
    };
    for source in ["", "abc", "привет 😀\r\n"] {
        std::fs::write(&args.path, source).expect("write source");
        assert_eq!(run_with_color(&args, &config, false).expect("cat"), source);
    }
    args.path = dir.path().to_path_buf();
    assert!(
        run_with_color(&args, &config, false)
            .expect_err("directory")
            .contains("regular file")
    );
    args.path = dir.path().join("missing");
    assert!(
        run_with_color(&args, &config, false)
            .expect_err("missing file")
            .contains("cannot read")
    );
}

#[test]
fn protocol_negotiation_full_range_and_unsupported_fallback() {
    use crate::test_support::lsp_peer::LspPeer;
    use std::time::Duration;
    for provider in [
        Value::Null,
        json!({"legend":{"tokenTypes":["string"],"tokenModifiers":[]},"full":false}),
        json!({"legend":{"tokenTypes":["string"],"tokenModifiers":[]},"full":true,"range":true}),
        json!({"legend":{"tokenTypes":["string"],"tokenModifiers":[]},"full":{"delta":true}}),
        json!({"legend":{"tokenTypes":["string"],"tokenModifiers":[]},"range":true}),
    ] {
        let supported = provider.get("full").is_some_and(|full| full != false)
            || provider.get("range") == Some(&json!(true));
        let full = provider.get("full").is_some_and(|full| full != false);
        let dir = TestDir::new("cat-protocol");
        let path = dir.write_file("source.lua", "different on-disk snapshot");
        let (mut client, server) = LspPeer::spawn_lua(&dir, Duration::from_secs(2), move |peer| {
            let initialize = peer.read();
            let capabilities =
                &initialize["params"]["capabilities"]["textDocument"]["semanticTokens"];
            assert_eq!(capabilities["augmentsSyntaxTokens"], false);
            assert_eq!(capabilities["multilineTokenSupport"], false);
            assert_eq!(capabilities["overlappingTokenSupport"], false);
            assert_eq!(capabilities["requests"], json!({"full":true,"range":true}));
            let mut result = json!({"capabilities":{}});
            if !provider.is_null() {
                result["capabilities"]["semanticTokensProvider"] = provider;
            }
            peer.reply(&initialize, result);
            assert_eq!(peer.read()["method"], "initialized");
            if supported {
                let open = peer.read();
                assert_eq!(open["method"], "textDocument/didOpen");
                assert_eq!(open["params"]["textDocument"]["languageId"], "lua");
                assert_eq!(open["params"]["textDocument"]["text"], "head\n😀");
                let request = peer.read();
                assert_eq!(
                    request["method"],
                    if full {
                        "textDocument/semanticTokens/full"
                    } else {
                        "textDocument/semanticTokens/range"
                    }
                );
                if !full {
                    assert_eq!(
                        request["params"]["range"],
                        json!({"start":{"line":0,"character":0},"end":{"line":1,"character":2}})
                    );
                }
                peer.reply(&request, json!({"data":[1,0,2,0,0]}));
            }
            peer.finish();
        });
        let initialize = client
            .initialize("file:///workspace", "workspace", false)
            .expect("initialize");
        let output =
            super::highlight_document(&mut client, &path, "head\n😀", &initialize).expect("cat");
        assert_eq!(
            output,
            if supported {
                "head\n\x1b[32m😀\x1b[0m"
            } else {
                "head\n😀"
            }
        );
        client.shutdown().expect("shutdown");
        server.join().expect("server");
    }
}
