use crate::config::{DocumentLanguages, load_config_store};
use crate::lsp::path_to_file_uri;
use crate::test_support::{TestDir, lsp_peer::LspPeer};
use serde_json::json;
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;

#[test]
fn sends_configured_ids_in_direct_and_windowed_document_opens() {
    let cases = [
        ("lua", "main.lua", "lua"),
        ("cs", "main.cs", "csharp"),
        ("python", "main.PY", "python"),
        ("bash", ".bashrc", "shellscript"),
        ("objc", "main.m", "objective-c"),
        ("typescriptreact", "main.tsx", "typescriptreact"),
        ("make", "Makefile", "makefile"),
    ];
    let config = load_config_store(&Path::new(env!("CARGO_MANIFEST_DIR")).join("data"))
        .expect("pinned catalog");
    let allowed = cases
        .iter()
        .map(|(id, _, _)| (*id).to_string())
        .collect::<BTreeSet<_>>();
    let languages = DocumentLanguages::new(&config.filetypes, &allowed);
    for windowed in [false, true] {
        let dir = TestDir::new("document-language-frames");
        let paths = cases
            .iter()
            .map(|(_, name, _)| dir.write_file(name, "sample\n"))
            .collect::<Vec<_>>();
        let (client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), move |peer| {
            for (_, filename, language) in cases {
                let open = peer.read();
                assert_eq!(open["method"], "textDocument/didOpen");
                let document = &open["params"]["textDocument"];
                assert_eq!(document["languageId"], language, "{filename}");
                assert!(document["uri"].as_str().expect("uri").ends_with(filename));
                if windowed {
                    let symbols = peer.read();
                    assert_eq!(symbols["method"], "textDocument/documentSymbol");
                    peer.reply(&symbols, json!([]));
                }
            }
            peer.finish();
        });
        let mut client = client.with_document_languages(languages.clone());
        if windowed {
            client
                .document_symbols_window(
                    &paths,
                    NonZeroUsize::new(2).expect("positive window"),
                    |_, _| Ok(()),
                )
                .expect("windowed documents");
        } else {
            for path in &paths {
                let uri = path_to_file_uri(path).expect("uri");
                client.open_document(path, &uri).expect("direct document");
                client
                    .open_document(path, &uri)
                    .expect("duplicate open is suppressed");
            }
        }
        client.shutdown().expect("shutdown");
        server.join().expect("server finishes");
    }
}
