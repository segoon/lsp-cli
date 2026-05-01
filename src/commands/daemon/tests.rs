use super::{
    BackgroundWorkTracker, fingerprint_value, normalize_initialize_params,
    update_background_work_tracker, wants_background_work,
};
use crate::runtime_state::daemon_socket_path;
use crate::test_support::TestDir;
use serde_json::json;

fn daemon_target(dir: &TestDir) -> super::DaemonTarget {
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
