use crate::config::{DocumentLanguages, load_config_store};
use crate::lsp::path_to_file_uri;
use crate::test_support::{TestDir, lsp_peer::LspPeer};
use serde_json::json;
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;

fn languages(ids: impl IntoIterator<Item = &'static str>) -> DocumentLanguages {
    let config = load_config_store(&Path::new(env!("CARGO_MANIFEST_DIR")).join("data"))
        .expect("pinned catalog");
    DocumentLanguages::new(
        &config.filetypes,
        &ids.into_iter().map(str::to_string).collect(),
    )
}

#[test]
fn sends_configured_ids_in_direct_and_windowed_document_opens() {
    enum OpenMode {
        Direct,
        Windowed,
        Server,
    }
    let cases = [
        ("lua", "main.lua", "lua"),
        ("cs", "main.cs", "csharp"),
        ("python", "main.PY", "python"),
        ("bash", ".bashrc", "shellscript"),
        ("objc", "main.m", "objective-c"),
        ("typescriptreact", "main.tsx", "typescriptreact"),
        ("make", "Makefile", "makefile"),
    ];
    let languages = languages(cases.iter().map(|(id, _, _)| *id));
    for mode in [OpenMode::Direct, OpenMode::Windowed, OpenMode::Server] {
        let windowed = matches!(mode, OpenMode::Windowed);
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
                for _ in 0..2 {
                    match mode {
                        OpenMode::Server => client.open_server_document(path, &uri),
                        _ => client.open_document(path, &uri),
                    }
                    .expect("document opens once with its configured language");
                }
            }
        }
        client.shutdown().expect("shutdown");
        server.join().expect("server finishes");
    }
}

#[test]
fn only_server_locations_can_skip_unknown_languages_and_ambiguity_still_fails() {
    for (filename, ambiguous) in [("unknown.generated", false), ("ambiguous.m", true)] {
        let dir = TestDir::new("server-document-language");
        let path = dir.write_file(filename, "sample\n");
        let uri = path_to_file_uri(&path).expect("uri");
        let (client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), LspPeer::finish);
        let mut client = client.with_document_languages(languages(["objc", "matlab"]));
        let result = client.open_server_document(&path, &uri);
        if ambiguous {
            assert!(
                result
                    .expect_err("ambiguous server location")
                    .contains("different language IDs")
            );
        } else {
            result.expect("leave unknown server document closed");
            assert!(!client.opened_documents.contains(&uri));
        }
        let error = client
            .open_document(&path, &uri)
            .expect_err("strict document opening");
        assert!(error.contains(if ambiguous {
            "different language IDs"
        } else {
            "no filetype matches"
        }));
        client.shutdown().expect("shutdown");
        server.join().expect("no didOpen was sent");
    }
}
