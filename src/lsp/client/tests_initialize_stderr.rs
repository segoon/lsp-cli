use super::{ClientTransport, LspClient};
use crate::server_stderr::CapturedStderr;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Cursor;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::Duration;

#[cfg(unix)]
#[test]
fn reports_server_stderr_for_initialize_transport_errors() {
    let (stream, _) = UnixStream::pair().expect("socket pair should open");
    let (_sender, receiver) = mpsc::channel();
    let stderr = CapturedStderr::spawn(
        Cursor::new(b"No ast-grep project configuration is found.\n".to_vec()),
        false,
    );
    let mut client = LspClient {
        transport: ClientTransport::Socket { stream },
        stderr: Some(stderr),
        messages: receiver,
        pending_messages: VecDeque::new(),
        next_request_id: 1,
        shutdown_sent: false,
        opened_documents: BTreeSet::new(),
        workspace_folders: None,
        published_diagnostics: BTreeMap::new(),
        process_exit_logged: true,
        debug: false,
        timeout: Duration::from_secs(1),
    };

    let error = client.format_transport_wait_error("initialize", "initialize failed".to_string());

    assert!(error.contains("initialize failed"));
    assert!(error.contains("No ast-grep project configuration is found."));
}
