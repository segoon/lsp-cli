use super::{
    CallHierarchyDirection, CallHierarchyQueryContext, LocationQueryKind, NamedAnchorRequest,
    NamedLocationQueryContext, PreparedWorkspace, collect_call_hierarchy_matches,
    collect_named_location_matches, select_named_anchors,
};
use crate::config::ConfigStore;
use crate::config::{CliConfig, FiletypeConfig};
use crate::lsp::{SymbolMatch, path_to_file_uri};
use crate::test_support::{TestDir, detection_result, lsp_peer::LspPeer, suggested_language};
use lsp_types::SymbolKind;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::time::Duration;

struct Fixture {
    dir: TestDir,
    workspace: PreparedWorkspace,
    config: ConfigStore,
    initialize: crate::lsp::InitializeResponse,
}

impl Fixture {
    fn new(limit: usize) -> Self {
        let dir = TestDir::new("named-window");
        for file in ["a.lua", "b.lua", "c.lua"] {
            dir.write_file(file, "local target = 1\n");
        }
        let workspace = PreparedWorkspace {
            detection: detection_result(&["lua"], &[]),
            server: suggested_language("fake", "fake", "fake", "lua"),
            allowed_filetypes: BTreeSet::from(["lua".into()]),
            root_uri: path_to_file_uri(dir.path()).expect("uri"),
            workspace_name: "test".into(),
            daemon_socket_path: None,
            daemon_socket_error: None,
        };
        let config = ConfigStore {
            filetypes: vec![FiletypeConfig {
                id: "lua".into(),
                extensions: vec!["lua".into()],
                patterns: vec![],
            }],
            lsps: vec![],
            cli: CliConfig {
                max_requests_in_flight: NonZeroUsize::new(limit),
                ..Default::default()
            },
        };
        let initialize = crate::lsp::InitializeResponse::from_raw_value(json!({"capabilities": {
            "workspaceSymbolProvider":true, "documentSymbolProvider":true,
            "referencesProvider":true, "definitionProvider":true, "declarationProvider":true,
            "callHierarchyProvider":true
        }}))
        .expect("capabilities");
        Self {
            dir,
            workspace,
            config,
            initialize,
        }
    }

    fn request(&self, function_only: bool) -> NamedAnchorRequest<'_> {
        NamedAnchorRequest {
            directory: self.dir.path(),
            name: "target",
            function_only,
        }
    }
}

fn range() -> Value {
    json!({"start":{"line":0,"character":6},"end":{"line":0,"character":12}})
}

fn serve_symbols(peer: &mut LspPeer, limit: usize, matching: bool) {
    let mut remaining = 3;
    while remaining > 0 {
        let requests = (0..limit.min(remaining))
            .map(|_| peer.document_request())
            .collect::<Vec<_>>();
        for request in requests.iter().rev() {
            let variable = request["params"]["textDocument"]["uri"]
                .as_str()
                .expect("uri")
                .ends_with("c.lua");
            let symbol = json!({"name":if matching {"target"} else {"other"},
                "kind":if variable {13} else {12}, "range":range(), "selectionRange":range()});
            peer.reply(request, json!([symbol.clone(), symbol]));
        }
        remaining -= requests.len();
    }
}

#[test]
fn named_location_queries_find_local_and_duplicate_names_in_file_order() {
    for kind in [
        LocationQueryKind::References,
        LocationQueryKind::Definition,
        LocationQueryKind::Declaration,
    ] {
        for limit in [1, 20] {
            let fixture = Fixture::new(limit);
            let (mut client, server) =
                LspPeer::spawn(&fixture.dir, Duration::from_secs(3), move |peer| {
                    let workspace = peer.read();
                    assert_eq!(workspace["method"], "workspace/symbol");
                    peer.reply(&workspace, Value::Null);
                    serve_symbols(peer, limit, true);
                    for file in ["a.lua", "b.lua", "c.lua"] {
                        let request = peer.read();
                        assert_eq!(request["method"], format!("textDocument/{}", kind.label()));
                        let uri = &request["params"]["textDocument"]["uri"];
                        assert!(uri.as_str().expect("uri").ends_with(file));
                        assert_eq!(
                            request["params"]["position"],
                            json!({"line":0,"character":6})
                        );
                        peer.reply(&request, json!([{"uri":uri, "range":range()}]));
                    }
                    peer.finish();
                });
            let matches = collect_named_location_matches(
                &fixture.workspace,
                &fixture.initialize,
                &mut client,
                NamedLocationQueryContext {
                    config: &fixture.config,
                    directory: fixture.dir.path(),
                    name: "target",
                    kind,
                    include_full_content: false,
                },
            )
            .expect("named query");
            assert_eq!(
                matches
                    .iter()
                    .map(|m| m.path.file_name().expect("file").to_str().expect("utf8"))
                    .collect::<Vec<_>>(),
                ["a.lua", "b.lua", "c.lua"]
            );
            assert!(
                matches
                    .iter()
                    .all(|m| m.name == "target" && m.line == 1 && m.col == 7)
            );
            client.shutdown().expect("shutdown");
            server.join().expect("server finishes");
        }
    }
}

#[test]
fn call_hierarchy_queries_filter_non_functions() {
    for direction in [
        CallHierarchyDirection::Incoming,
        CallHierarchyDirection::Outgoing,
    ] {
        let fixture = Fixture::new(20);
        let (mut client, server) = LspPeer::spawn(
            &fixture.dir,
            Duration::from_secs(3),
            move |peer| {
                let workspace = peer.read();
                peer.reply(&workspace, Value::Null);
                serve_symbols(peer, 20, true);
                for file in ["a.lua", "b.lua"] {
                    let prepare = peer.read();
                    assert_eq!(prepare["method"], "textDocument/prepareCallHierarchy");
                    let uri = &prepare["params"]["textDocument"]["uri"];
                    assert!(uri.as_str().expect("uri").ends_with(file));
                    let item = json!({"name":"target", "kind":12, "uri":uri, "range":range(), "selectionRange":range()});
                    peer.reply(&prepare, json!([item]));
                    let calls = peer.read();
                    let method = match direction {
                        CallHierarchyDirection::Incoming => "callHierarchy/incomingCalls",
                        CallHierarchyDirection::Outgoing => "callHierarchy/outgoingCalls",
                    };
                    assert_eq!(calls["method"], method);
                    peer.reply(&calls, json!([]));
                }
                peer.finish();
            },
        );
        let matches = collect_call_hierarchy_matches(
            &fixture.workspace,
            &fixture.initialize,
            &mut client,
            CallHierarchyQueryContext {
                config: &fixture.config,
                directory: fixture.dir.path(),
                name: "target",
                direction,
            },
        )
        .expect("call hierarchy");
        assert!(matches.is_empty());
        client.shutdown().expect("shutdown");
        server.join().expect("server finishes");
    }
}

#[test]
fn falls_back_to_workspace_symbols_when_no_document_name_matches() {
    let fixture = Fixture::new(20);
    let anchor = SymbolMatch {
        name: "target".into(),
        kind: SymbolKind::FUNCTION,
        path: fixture.dir.path().join("a.lua"),
        line: 1,
        col: 7,
        line_content: "local target = 1".into(),
        full_content: None,
    };
    let (mut client, server) = LspPeer::spawn(&fixture.dir, Duration::from_secs(3), |peer| {
        serve_symbols(peer, 20, false);
        peer.finish();
    });
    let matches = select_named_anchors(
        &fixture.workspace,
        &fixture.initialize,
        &mut client,
        &fixture.config,
        fixture.request(false),
        vec![anchor.clone()],
    )
    .expect("fallback");
    assert_eq!(matches, vec![anchor]);
    client.shutdown().expect("shutdown");
    server.join().expect("server finishes");
}
