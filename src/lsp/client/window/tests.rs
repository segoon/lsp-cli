use crate::error::Error;
use crate::test_support::{TestDir, lsp_peer::LspPeer};
use serde_json::{Value, json};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

fn files(dir: &TestDir, count: usize) -> Vec<PathBuf> {
    (0..count)
        .map(|index| dir.write_file(&format!("{index}.lua"), "local target = 1\n"))
        .collect()
}

fn window(size: usize) -> NonZeroUsize {
    NonZeroUsize::new(size).expect("positive test size")
}

fn echo_file(peer: &mut LspPeer, request: &Value) {
    peer.reply(request, request["params"]["textDocument"]["uri"].clone());
}

#[test]
fn fills_window_and_refills_after_out_of_order_reply() {
    let dir = TestDir::new("window-refill");
    let paths = files(&dir, 4);
    let (mut client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), |peer| {
        let first = peer.document_request();
        let second = peer.document_request();
        echo_file(peer, &second);
        // Refill must arrive while the first request is still unanswered.
        let third = peer.document_request();
        echo_file(peer, &third);
        let fourth = peer.document_request();
        echo_file(peer, &fourth);
        echo_file(peer, &first);
        peer.finish();
    });
    let results = client
        .document_symbols_window(&paths, window(2), |path, response| {
            assert_eq!(*response, crate::lsp::path_to_file_uri(path).expect("uri"));
            Ok(path.to_path_buf())
        })
        .expect("window completes");
    assert_eq!(results, paths);
    client.shutdown().expect("shutdown");
    server.join().expect("server finishes");
}

#[test]
fn respects_window_limit_with_interleaved_server_requests() {
    for size in [1, 3, 20] {
        let dir = TestDir::new("window-bound");
        let paths = files(&dir, 23);
        let (mut client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), move |peer| {
            let mut remaining = 23;
            while remaining > 0 {
                let group = (0..size.min(remaining))
                    .map(|_| peer.document_request())
                    .collect::<Vec<_>>();
                // If the client overfilled the window, its next message would be
                // another document request rather than this server-request reply.
                peer.send(&json!({"jsonrpc":"2.0", "id":"probe", "method":"workspace/configuration", "params":{"items":[]}}));
                let reply = peer.read();
                assert_eq!(reply["id"], "probe");
                assert!(reply.get("result").is_some());
                for request in group.iter().rev() {
                    echo_file(peer, request);
                }
                remaining -= group.len();
            }
            peer.finish();
        });
        let results = client
            .document_symbols_window(&paths, window(size), |path, _| Ok(path.to_path_buf()))
            .expect("window");
        assert_eq!(results, paths);
        client.shutdown().expect("shutdown");
        server.join().expect("server finishes");
    }
}

#[test]
fn skips_explicit_errors_and_keeps_notifications() {
    let dir = TestDir::new("window-errors");
    let paths = files(&dir, 3);
    let (mut client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), |peer| {
        let first = peer.document_request();
        let second = peer.document_request();
        peer.send(&json!({"jsonrpc":"2.0", "method":"textDocument/publishDiagnostics", "params":{"uri":"file:///sample.lua", "diagnostics":[]}}));
        peer.send(&json!({"jsonrpc":"2.0", "id":first["id"], "error":{"code":-32603,"message":"unsupported document"}}));
        let third = peer.document_request();
        echo_file(peer, &third);
        echo_file(peer, &second);
        peer.finish();
    });
    let results = client
        .document_symbols_window(&paths, window(2), |path, _| Ok(path.to_path_buf()))
        .expect("window");
    assert_eq!(results, paths[1..]);
    assert_eq!(client.published_diagnostics_len(), 1);
    client.shutdown().expect("shutdown");
    server.join().expect("server finishes");
}

#[test]
fn timeout_cancels_outstanding_requests_and_ignores_late_responses() {
    let dir = TestDir::new("window-timeout");
    let paths = files(&dir, 3);
    let (mut client, server) = LspPeer::spawn(&dir, Duration::from_millis(120), |peer| {
        let first = peer.document_request();
        let second = peer.document_request();
        // Traffic lasts longer than the deadline. A timeout restarted by each
        // notification would incorrectly allow the later successful responses.
        for _ in 0..30 {
            peer.send(&json!({"jsonrpc":"2.0","method":"window/logMessage","params":{"type":3,"message":"busy"}}));
            thread::sleep(Duration::from_millis(5));
        }
        for request in [&first, &second] {
            let cancel = peer.read();
            assert_eq!(cancel["method"], "$/cancelRequest");
            assert_eq!(cancel["params"]["id"], request["id"]);
            echo_file(peer, request);
        }
        peer.finish();
    });
    let error = client
        .document_symbols_window(&paths, window(2), |_, _| Ok(()))
        .expect_err("timeout");
    assert!(error.contains("timed out waiting for document symbols"));
    assert!(error.contains("0.lua"));
    client
        .shutdown()
        .expect("late replies do not satisfy shutdown");
    server.join().expect("server finishes");
}

#[test]
fn decode_failure_cancels_other_requests() {
    let dir = TestDir::new("window-decode");
    let paths = files(&dir, 3);
    let (mut client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), |peer| {
        let first = peer.document_request();
        let second = peer.document_request();
        peer.reply(&first, Value::Null);
        let cancel = peer.read();
        assert_eq!(cancel["method"], "$/cancelRequest");
        assert_eq!(cancel["params"]["id"], second["id"]);
        echo_file(peer, &second);
        peer.finish();
    });
    let error = client
        .document_symbols_window::<()>(&paths, window(2), |_, _| Err(Error::lsp("invalid symbols")))
        .expect_err("decode failure");
    assert!(error.contains("invalid symbols"));
    client.shutdown().expect("shutdown");
    server.join().expect("server finishes");
}

#[test]
fn opening_failure_cancels_requests_already_sent() {
    let dir = TestDir::new("window-missing");
    let mut paths = files(&dir, 1);
    paths.push(dir.path().join("missing.lua"));
    let (mut client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), |peer| {
        let request = peer.document_request();
        let cancel = peer.read();
        assert_eq!(cancel["method"], "$/cancelRequest");
        assert_eq!(cancel["params"]["id"], request["id"]);
        peer.finish();
    });
    let error = client
        .document_symbols_window(&paths, window(2), |_, _| Ok(()))
        .expect_err("missing file");
    assert!(error.contains("missing.lua"));
    client.shutdown().expect("shutdown");
    server.join().expect("server finishes");
}

#[test]
fn disconnect_aborts_discovery() {
    let dir = TestDir::new("window-disconnect");
    let paths = files(&dir, 3);
    let (mut client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), |peer| {
        peer.document_request();
        peer.document_request();
    });
    client
        .document_symbols_window(&paths, window(2), |_, _| Ok(()))
        .expect_err("disconnect aborts scan");
    server.join().expect("server finishes");
}

#[test]
fn empty_window_sends_no_requests() {
    let dir = TestDir::new("window-empty");
    let (mut client, server) = LspPeer::spawn(&dir, Duration::from_secs(3), LspPeer::finish);
    let results = client
        .document_symbols_window(&[], window(20), |_, _| Ok(()))
        .expect("empty scan");
    assert!(results.is_empty());
    client.shutdown().expect("shutdown");
    server.join().expect("server finishes");
}
