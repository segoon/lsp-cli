use super::protocol::{
    fingerprint_value, normalize_initialize_params, update_background_work_tracker,
    wants_background_work,
};
use super::{BackgroundWorkTracker, StopSocketResult, stop_socket};
use crate::lsp::transport::{read_message, write_message};
use crate::runtime_state::daemon_socket_path;
use crate::test_support::TestDir;
use serde_json::json;
use std::fs;
use std::io::BufReader;
use std::os::unix::net::UnixListener;
use std::thread;

pub(super) fn daemon_target(dir: &TestDir) -> super::DaemonTarget {
    let workspace_root = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace_root).expect("workspace should exist");

    super::DaemonTarget {
        path: workspace_root.clone(),
        workspace_root_string: workspace_root.display().to_string(),
        root_uri: crate::lsp::path_to_file_uri(&workspace_root).expect("uri should build"),
        workspace_name: crate::lsp::workspace_name(&workspace_root),
        server_name: "rust-analyzer".to_string(),
        socket_path: dir.path().join("daemon.sock"),
    }
}

#[test]
fn socket_path_changes_with_workspace_and_command() {
    let dir = TestDir::new("daemon-socket");
    let socket_root = dir.path().join("run");
    let first = daemon_socket_path(
        &socket_root,
        &dir.path().join("one"),
        "rust-analyzer",
        &["rust-analyzer".to_string()],
    );
    let second = daemon_socket_path(
        &socket_root,
        &dir.path().join("two"),
        "rust-analyzer",
        &["rust-analyzer".to_string()],
    );
    let third = daemon_socket_path(
        &socket_root,
        &dir.path().join("one"),
        "rust-analyzer",
        &["rust-analyzer".to_string(), "--stdio".to_string()],
    );

    assert_ne!(first, second);
    assert_ne!(first, third);
}

#[test]
fn normalize_initialize_params_rewrites_process_id_and_workspace() {
    let dir = TestDir::new("daemon-normalize");
    let target = daemon_target(&dir);
    let params = json!({
        "processId": 1,
        "rootUri": target.root_uri,
        "rootPath": target.workspace_root_string,
        "workspaceFolders": [{"uri": target.root_uri, "name": "ignored"}],
        "workDoneToken": "abc",
        "capabilities": {"workspace": {"configuration": true}},
    });

    let normalized =
        normalize_initialize_params(&params, &target).expect("params should normalize");

    assert_eq!(
        normalized
            .get("rootUri")
            .and_then(serde_json::Value::as_str),
        Some(target.root_uri.as_str())
    );
    assert_eq!(
        normalized
            .get("rootPath")
            .and_then(serde_json::Value::as_str),
        Some(target.workspace_root_string.as_str())
    );
    assert!(normalized.get("workDoneToken").is_none());
}

#[test]
fn normalize_initialize_params_rejects_other_workspace() {
    let dir = TestDir::new("daemon-normalize");
    let target = daemon_target(&dir);
    let error = normalize_initialize_params(
        &json!({
            "rootUri": "file:///elsewhere",
        }),
        &target,
    )
    .expect_err("mismatched workspace should fail");

    assert!(error.contains("rootUri"));
}

#[test]
fn fingerprint_value_sorts_object_keys() {
    let left = json!({"b": 1, "a": [true, null]});
    let right = json!({"a": [true, null], "b": 1});

    assert_eq!(fingerprint_value(&left), fingerprint_value(&right));
}

#[test]
fn tracks_background_work_until_progress_completes() {
    let mut tracker = BackgroundWorkTracker::default();

    update_background_work_tracker(
        &json!({
            "jsonrpc": "2.0",
            "method": "$/progress",
            "params": {
                "token": "rust-analyzer/flycheck",
                "value": {"kind": "begin"}
            }
        }),
        &mut tracker,
    )
    .expect("progress begin should decode");
    assert_eq!(
        tracker.state,
        super::protocol::BackgroundWorkState::InProgress
    );

    update_background_work_tracker(
        &json!({
            "jsonrpc": "2.0",
            "method": "$/progress",
            "params": {
                "token": "rust-analyzer/flycheck",
                "value": {"kind": "end"}
            }
        }),
        &mut tracker,
    )
    .expect("progress end should decode");

    assert_eq!(
        tracker.state,
        super::protocol::BackgroundWorkState::Quiescent
    );
}

#[test]
fn tracks_quiescent_server_status_from_upstream() {
    let mut tracker = BackgroundWorkTracker::default();

    update_background_work_tracker(
        &json!({
            "jsonrpc": "2.0",
            "method": "experimental/serverStatus",
            "params": {
                "health": "ok",
                "quiescent": true,
                "message": null
            }
        }),
        &mut tracker,
    )
    .expect("server status should decode");

    assert_eq!(
        tracker.state,
        super::protocol::BackgroundWorkState::Quiescent
    );
}

#[test]
fn detects_background_work_capabilities_in_initialize_params() {
    assert!(wants_background_work(&json!({
        "capabilities": {
            "window": {"workDoneProgress": true}
        }
    })));
    assert!(wants_background_work(&json!({
        "capabilities": {
            "experimental": {"serverStatusNotification": true}
        }
    })));
    assert!(!wants_background_work(&json!({
        "capabilities": {
            "window": {"workDoneProgress": false}
        }
    })));
}

#[test]
fn emits_quiescent_notification_for_reused_warm_sessions() {
    assert_eq!(
        super::protocol::background_work_ready_notification(),
        json!({
            "jsonrpc": "2.0",
            "method": "experimental/serverStatus",
            "params": {
                "health": "ok",
                "quiescent": true,
                "message": null,
            }
        })
    );
}

#[test]
fn stop_socket_sends_private_stop_request() {
    let dir = TestDir::new("daemon-stop-socket");
    let socket_path = dir.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("client should connect");
        let reader_stream = stream.try_clone().expect("stream should clone");
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;
        let request = read_message(&mut reader)
            .expect("request should parse")
            .expect("request should exist");
        assert_eq!(
            request.get("method").and_then(serde_json::Value::as_str),
            Some(super::protocol::STOP_METHOD)
        );
        let response = json!({
            "jsonrpc": "2.0",
            "id": request.get("id").cloned().expect("request id should exist"),
            "result": null,
        });
        write_message(&mut writer, &response).expect("response should write");
    });

    assert!(matches!(
        stop_socket(&socket_path, false).expect("stop should succeed"),
        StopSocketResult::Stopped
    ));
    server.join().expect("server thread should finish");
}

#[test]
fn stop_socket_removes_stale_socket() {
    let dir = TestDir::new("daemon-stop-stale");
    let socket_path = dir.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");
    drop(listener);

    assert!(matches!(
        stop_socket(&socket_path, false).expect("stale cleanup should succeed"),
        StopSocketResult::RemovedStaleSocket
    ));
    assert!(!socket_path.exists(), "stale socket should be removed");
}

#[test]
fn stop_socket_returns_not_running_when_socket_is_missing() {
    let dir = TestDir::new("daemon-stop-missing");
    let socket_path = dir.path().join("daemon.sock");
    fs::create_dir_all(dir.path()).expect("temp dir should exist");

    assert!(matches!(
        stop_socket(&socket_path, false).expect("missing socket should not fail"),
        StopSocketResult::NotRunning
    ));
}

#[test]
fn forwards_multiple_requests_and_out_of_order_responses() {
    use super::{ClientPhase, ClientSession, Daemon, ReaderEvent, UpstreamServer};
    use crate::server_stderr::CapturedStderr;
    use std::collections::{BTreeMap, BTreeSet};
    use std::os::unix::net::UnixStream;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let dir = TestDir::new("daemon-window");
    let target = daemon_target(&dir);
    // Echo upstream bytes so the test can inspect forwarding without a real LSP.
    let mut child = Command::new("/bin/cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start echo process");
    let upstream = UpstreamServer {
        stdin: child.stdin.take().expect("stdin"),
        messages: super::process::spawn_reader(child.stdout.take().expect("stdout")),
        stderr: CapturedStderr::spawn(child.stderr.take().expect("stderr"), false),
        child,
        initialize_fingerprint: None,
        initialize_result: None,
        restart_required: false,
        background_work: BackgroundWorkTracker::default(),
    };
    let (client, proxy) = UnixStream::pair().expect("client socket pair");
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("read timeout");
    let mut session = ClientSession::new(proxy).expect("session");
    session.phase = ClientPhase::Ready;
    let mut daemon = Daemon {
        listener: UnixListener::bind(&target.socket_path).expect("listener"),
        target,
        debug: false,
        idle_timeout: Duration::from_secs(60),
        upstream: Some(upstream),
        active_client: Some(session),
        orphaned_client_requests: BTreeSet::new(),
        idle_since: Instant::now(),
        stop_requested: false,
    };
    let mut expected = BTreeMap::new();
    for id in 1..=20 {
        let request = json!({"jsonrpc":"2.0","id":id,"method":"textDocument/documentSymbol", "params":{"textDocument":{"uri":format!("file:///{id}.lua")}}});
        daemon
            .handle_client_message(&request)
            .expect("forward request");
        match daemon
            .upstream
            .as_ref()
            .expect("upstream")
            .messages
            .recv_timeout(Duration::from_secs(3))
            .expect("echo")
        {
            ReaderEvent::Message(message) => assert_eq!(message, request),
            _ => panic!("expected echoed request"),
        }
        expected.insert(id, json!({"jsonrpc":"2.0","id":id,"result":[]}));
    }
    assert_eq!(
        daemon
            .active_client
            .as_ref()
            .expect("client")
            .forwarded_client_requests
            .len(),
        20
    );
    let mut reader = BufReader::new(client);
    for response in expected.values().rev() {
        daemon
            .handle_upstream_message(response)
            .expect("forward response");
        assert_eq!(
            read_message(&mut reader).expect("read").expect("response"),
            *response
        );
    }
    assert!(
        daemon
            .active_client
            .as_ref()
            .expect("client")
            .forwarded_client_requests
            .is_empty()
    );
    // Child handles do not stop/reap processes automatically; finish the echo helper.
    let upstream = daemon.upstream.as_mut().expect("upstream");
    upstream.child.kill().expect("stop echo process");
    upstream.child.wait().expect("reap echo process");
}
