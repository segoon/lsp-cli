use super::{ClientTransport, LspClient, format_spawn_error};
#[cfg(unix)]
use crate::lsp::transport::{read_message, write_message};
use crate::test_support::TestDir;
#[cfg(unix)]
use serde_json::json;
use std::fs;
use std::time::Duration;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::io::BufReader;
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
#[cfg(unix)]
use std::sync::{Mutex, OnceLock};
#[cfg(unix)]
use std::thread;

#[test]
fn formats_missing_binary_error() {
    let error = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");

    assert_eq!(
        format_spawn_error("ast-grep", &error),
        "LSP server executable `ast-grep` is not installed or not in $PATH"
    );
}

#[cfg(unix)]
#[test]
fn starts_server_in_workspace_root() {
    let dir = TestDir::new("client");
    let workspace_root = dir.path().join("workspace");
    fs::create_dir_all(&workspace_root).expect("workspace should be created");
    let cwd_file = dir.path().join("cwd.txt");
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "pwd > \"$1\"".to_string(),
        "sh".to_string(),
        cwd_file.display().to_string(),
    ];

    let mut client = LspClient::new(&command, &workspace_root, false, Duration::from_secs(1))
        .expect("helper process should start");
    let status = match &mut client.transport {
        ClientTransport::Process { child, .. } => child.wait().expect("helper process should exit"),
        ClientTransport::Socket { .. } => panic!("expected process transport"),
    };

    assert!(status.success());
    assert_eq!(
        fs::read_to_string(&cwd_file)
            .expect("cwd file should be written")
            .trim_end(),
        workspace_root.display().to_string()
    );
}

#[cfg(unix)]
#[test]
fn hides_server_stderr_without_debug() {
    assert_eq!(captured_server_stderr(false), "");
}

#[cfg(unix)]
#[test]
fn keeps_server_stderr_visible_with_debug() {
    assert!(captured_server_stderr(true).contains("server stderr\n"));
}

#[cfg(unix)]
#[test]
fn initialize_replies_to_queued_server_requests_before_next_request() {
    let dir = TestDir::new("client-init-queue");
    let socket_path = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("client should connect");
        let reader_stream = stream.try_clone().expect("stream should clone");
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;

        let initialize = read_message(&mut reader)
            .expect("initialize should parse")
            .expect("initialize should exist");
        assert_eq!(
            initialize.get("method").and_then(serde_json::Value::as_str),
            Some("initialize")
        );
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": initialize.get("id").cloned().expect("initialize id should exist"),
                "result": {
                    "capabilities": {
                        "documentSymbolProvider": true,
                    }
                },
            }),
        )
        .expect("initialize response should write");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": "register-1",
                "method": "client/registerCapability",
                "params": {
                    "registrations": [{
                        "id": "watcher",
                        "method": "workspace/didChangeWatchedFiles",
                        "registerOptions": {
                            "watchers": [{"globPattern": "**/*", "kind": 4}]
                        }
                    }]
                }
            }),
        )
        .expect("registerCapability request should write");

        let initialized = read_message(&mut reader)
            .expect("initialized should parse")
            .expect("initialized should exist");
        assert_eq!(
            initialized
                .get("method")
                .and_then(serde_json::Value::as_str),
            Some("initialized")
        );

        let register_response = read_message(&mut reader)
            .expect("register response should parse")
            .expect("register response should exist");
        assert_eq!(
            register_response
                .get("id")
                .and_then(serde_json::Value::as_str),
            Some("register-1")
        );
        assert!(register_response.get("result").is_some());

        let shutdown = read_message(&mut reader)
            .expect("shutdown should parse")
            .expect("shutdown should exist");
        assert_eq!(
            shutdown.get("method").and_then(serde_json::Value::as_str),
            Some("shutdown")
        );
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": shutdown.get("id").cloned().expect("shutdown id should exist"),
                "result": null,
            }),
        )
        .expect("shutdown response should write");

        let exit = read_message(&mut reader)
            .expect("exit should parse")
            .expect("exit should exist");
        assert_eq!(
            exit.get("method").and_then(serde_json::Value::as_str),
            Some("exit")
        );
    });

    let mut client =
        LspClient::connect_unix(&socket_path, false, Duration::from_secs(1)).expect("connect");
    client
        .initialize("file:///workspace", "workspace", false)
        .expect("initialize should succeed");
    client.shutdown().expect("shutdown should succeed");

    server.join().expect("server thread should finish");
}

#[cfg(unix)]
#[test]
fn initialize_advertises_and_returns_workspace_folders() {
    let dir = TestDir::new("client-init-workspace-folders");
    let socket_path = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("client should connect");
        let reader_stream = stream.try_clone().expect("stream should clone");
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;

        let initialize = read_message(&mut reader)
            .expect("initialize should parse")
            .expect("initialize should exist");
        let params = initialize
            .get("params")
            .cloned()
            .expect("initialize params should exist");
        assert_eq!(
            params
                .get("capabilities")
                .and_then(|value| value.get("workspace"))
                .and_then(|value| value.get("workspaceFolders"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        let workspace_folders = params
            .get("workspaceFolders")
            .cloned()
            .expect("workspaceFolders should exist");

        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": initialize.get("id").cloned().expect("initialize id should exist"),
                "result": {
                    "capabilities": {
                        "documentSymbolProvider": true,
                    }
                },
            }),
        )
        .expect("initialize response should write");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": "folders-1",
                "method": "workspace/workspaceFolders",
            }),
        )
        .expect("workspaceFolders request should write");

        let initialized = read_message(&mut reader)
            .expect("initialized should parse")
            .expect("initialized should exist");
        assert_eq!(
            initialized
                .get("method")
                .and_then(serde_json::Value::as_str),
            Some("initialized")
        );

        let workspace_folders_response = read_message(&mut reader)
            .expect("workspaceFolders response should parse")
            .expect("workspaceFolders response should exist");
        assert_eq!(
            workspace_folders_response
                .get("id")
                .and_then(serde_json::Value::as_str),
            Some("folders-1")
        );
        assert_eq!(
            workspace_folders_response.get("result").cloned(),
            Some(workspace_folders)
        );

        let shutdown = read_message(&mut reader)
            .expect("shutdown should parse")
            .expect("shutdown should exist");
        assert_eq!(
            shutdown.get("method").and_then(serde_json::Value::as_str),
            Some("shutdown")
        );
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": shutdown.get("id").cloned().expect("shutdown id should exist"),
                "result": null,
            }),
        )
        .expect("shutdown response should write");

        let exit = read_message(&mut reader)
            .expect("exit should parse")
            .expect("exit should exist");
        assert_eq!(
            exit.get("method").and_then(serde_json::Value::as_str),
            Some("exit")
        );
    });

    let mut client =
        LspClient::connect_unix(&socket_path, false, Duration::from_secs(1)).expect("connect");
    client
        .initialize("file:///workspace", "workspace", false)
        .expect("initialize should succeed");
    client.shutdown().expect("shutdown should succeed");

    server.join().expect("server thread should finish");
}

#[cfg(unix)]
#[test]
fn collects_latest_publish_diagnostics_notifications() {
    let dir = TestDir::new("client-diagnostics");
    let socket_path = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("client should connect");
        let reader_stream = stream.try_clone().expect("stream should clone");
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;

        let initialize = read_message(&mut reader)
            .expect("initialize should parse")
            .expect("initialize should exist");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": initialize.get("id").cloned().expect("initialize id should exist"),
                "result": { "capabilities": {} },
            }),
        )
        .expect("initialize response should write");

        let initialized = read_message(&mut reader)
            .expect("initialized should parse")
            .expect("initialized should exist");
        assert_eq!(
            initialized
                .get("method")
                .and_then(serde_json::Value::as_str),
            Some("initialized")
        );

        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": {
                    "uri": "file:///workspace/src/main.rs",
                    "diagnostics": [{
                        "range": {
                            "start": {"line": 0, "character": 1},
                            "end": {"line": 0, "character": 2}
                        },
                        "message": "first"
                    }]
                }
            }),
        )
        .expect("first diagnostics should write");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": {
                    "uri": "file:///workspace/src/main.rs",
                    "diagnostics": [{
                        "range": {
                            "start": {"line": 1, "character": 1},
                            "end": {"line": 1, "character": 2}
                        },
                        "message": "second"
                    }]
                }
            }),
        )
        .expect("second diagnostics should write");

        let shutdown = read_message(&mut reader)
            .expect("shutdown should parse")
            .expect("shutdown should exist");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": shutdown.get("id").cloned().expect("shutdown id should exist"),
                "result": null,
            }),
        )
        .expect("shutdown response should write");

        let exit = read_message(&mut reader)
            .expect("exit should parse")
            .expect("exit should exist");
        assert_eq!(
            exit.get("method").and_then(serde_json::Value::as_str),
            Some("exit")
        );
    });

    let mut client =
        LspClient::connect_unix(&socket_path, false, Duration::from_secs(1)).expect("connect");
    client
        .initialize("file:///workspace", "workspace", false)
        .expect("initialize should succeed");
    client
        .collect_diagnostics(Duration::from_millis(100))
        .expect("collect should succeed");

    let diagnostics = client.take_published_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0]
            .get("params")
            .and_then(|value| value.get("diagnostics"))
            .and_then(|value| value.get(0))
            .and_then(|value| value.get("message"))
            .and_then(serde_json::Value::as_str),
        Some("second")
    );

    client.shutdown().expect("shutdown should succeed");
    server.join().expect("server thread should finish");
}

#[cfg(unix)]
#[test]
fn sends_document_diagnostic_request() {
    let dir = TestDir::new("client-document-diagnostic");
    let socket_path = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("client should connect");
        let reader_stream = stream.try_clone().expect("stream should clone");
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;

        let initialize = read_message(&mut reader)
            .expect("initialize should parse")
            .expect("initialize should exist");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": initialize.get("id").cloned().expect("initialize id should exist"),
                "result": {
                    "capabilities": {
                        "diagnosticProvider": {"interFileDependencies": false, "workspaceDiagnostics": false}
                    }
                },
            }),
        )
        .expect("initialize response should write");

        let initialized = read_message(&mut reader)
            .expect("initialized should parse")
            .expect("initialized should exist");
        assert_eq!(
            initialized
                .get("method")
                .and_then(serde_json::Value::as_str),
            Some("initialized")
        );

        let request = read_message(&mut reader)
            .expect("document diagnostic should parse")
            .expect("document diagnostic should exist");
        assert_eq!(
            request.get("method").and_then(serde_json::Value::as_str),
            Some("textDocument/diagnostic")
        );
        assert_eq!(
            request
                .get("params")
                .and_then(|value| value.get("textDocument"))
                .and_then(|value| value.get("uri"))
                .and_then(serde_json::Value::as_str),
            Some("file:///workspace/src/main.rs")
        );
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().expect("request id should exist"),
                "result": {"kind": "full", "items": []},
            }),
        )
        .expect("document diagnostic response should write");

        let shutdown = read_message(&mut reader)
            .expect("shutdown should parse")
            .expect("shutdown should exist");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": shutdown.get("id").cloned().expect("shutdown id should exist"),
                "result": null,
            }),
        )
        .expect("shutdown response should write");

        let exit = read_message(&mut reader)
            .expect("exit should parse")
            .expect("exit should exist");
        assert_eq!(
            exit.get("method").and_then(serde_json::Value::as_str),
            Some("exit")
        );
    });

    let mut client =
        LspClient::connect_unix(&socket_path, false, Duration::from_secs(1)).expect("connect");
    client
        .initialize("file:///workspace", "workspace", false)
        .expect("initialize should succeed");
    client
        .document_diagnostic("file:///workspace/src/main.rs")
        .expect("document diagnostic should succeed");
    client.shutdown().expect("shutdown should succeed");

    server.join().expect("server thread should finish");
}

#[cfg(unix)]
#[test]
fn sends_document_formatting_request() {
    let dir = TestDir::new("client-document-formatting");
    let socket_path = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("client should connect");
        let reader_stream = stream.try_clone().expect("stream should clone");
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;

        let initialize = read_message(&mut reader)
            .expect("initialize should parse")
            .expect("initialize should exist");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": initialize.get("id").cloned().expect("initialize id should exist"),
                "result": {
                    "capabilities": {
                        "documentFormattingProvider": true
                    }
                },
            }),
        )
        .expect("initialize response should write");

        let initialized = read_message(&mut reader)
            .expect("initialized should parse")
            .expect("initialized should exist");
        assert_eq!(
            initialized
                .get("method")
                .and_then(serde_json::Value::as_str),
            Some("initialized")
        );

        let request = read_message(&mut reader)
            .expect("format request should parse")
            .expect("format request should exist");
        assert_eq!(
            request.get("method").and_then(serde_json::Value::as_str),
            Some("textDocument/formatting")
        );
        assert_eq!(
            request
                .get("params")
                .and_then(|value| value.get("options"))
                .and_then(|value| value.get("tabSize"))
                .and_then(serde_json::Value::as_u64),
            Some(4)
        );
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().expect("request id should exist"),
                "result": [],
            }),
        )
        .expect("format response should write");

        let shutdown = read_message(&mut reader)
            .expect("shutdown should parse")
            .expect("shutdown should exist");
        write_message(
            &mut writer,
            &json!({
                "jsonrpc": "2.0",
                "id": shutdown.get("id").cloned().expect("shutdown id should exist"),
                "result": null,
            }),
        )
        .expect("shutdown response should write");

        let exit = read_message(&mut reader)
            .expect("exit should parse")
            .expect("exit should exist");
        assert_eq!(
            exit.get("method").and_then(serde_json::Value::as_str),
            Some("exit")
        );
    });

    let mut client =
        LspClient::connect_unix(&socket_path, false, Duration::from_secs(1)).expect("connect");
    client
        .initialize("file:///workspace", "workspace", false)
        .expect("initialize should succeed");
    client
        .format_document("file:///workspace/src/main.rs")
        .expect("format request should succeed");
    client.shutdown().expect("shutdown should succeed");

    server.join().expect("server thread should finish");
}

#[cfg(unix)]
fn captured_server_stderr(debug: bool) -> String {
    let _lock = stderr_lock()
        .lock()
        .expect("stderr lock should be available");
    let dir = TestDir::new("client-stderr");
    let workspace_root = dir.path().join("workspace");
    fs::create_dir_all(&workspace_root).expect("workspace should be created");
    let stderr_file = dir.path().join("stderr.txt");
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf 'server stderr\\n' >&2".to_string(),
    ];

    let mut client;
    {
        let _capture = StderrCapture::new(&stderr_file);
        client = LspClient::new(&command, &workspace_root, debug, Duration::from_secs(1))
            .expect("helper process should start");
        let status = match &mut client.transport {
            ClientTransport::Process { child, .. } => {
                child.wait().expect("helper process should exit")
            }
            ClientTransport::Socket { .. } => panic!("expected process transport"),
        };
        assert!(status.success());
    }

    drop(client);
    fs::read_to_string(stderr_file).expect("stderr capture should be readable")
}

#[cfg(unix)]
fn stderr_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(unix)]
struct StderrCapture {
    saved_stderr: i32,
}

#[cfg(unix)]
impl StderrCapture {
    fn new(path: &std::path::Path) -> Self {
        let mut stderr = std::io::stderr().lock();
        stderr.flush().expect("stderr should flush before capture");

        let file = File::create(path).expect("stderr capture file should be created");
        let saved_stderr = unsafe { dup(STDERR_FILENO) };
        assert!(saved_stderr >= 0, "stderr should be duplicated");

        let redirected = unsafe { dup2(file.as_raw_fd(), STDERR_FILENO) };
        assert!(redirected >= 0, "stderr should be redirected");
        drop(file);

        Self { saved_stderr }
    }
}

#[cfg(unix)]
impl Drop for StderrCapture {
    fn drop(&mut self) {
        let mut stderr = std::io::stderr().lock();
        stderr.flush().expect("stderr should flush before restore");

        let restored = unsafe { dup2(self.saved_stderr, STDERR_FILENO) };
        assert!(restored >= 0, "stderr should be restored");
        let closed = unsafe { close(self.saved_stderr) };
        assert_eq!(closed, 0, "saved stderr fd should be closed");
    }
}

#[cfg(unix)]
const STDERR_FILENO: i32 = 2;

#[cfg(unix)]
unsafe extern "C" {
    fn close(fd: i32) -> i32;
    fn dup(fd: i32) -> i32;
    fn dup2(src: i32, dst: i32) -> i32;
}
