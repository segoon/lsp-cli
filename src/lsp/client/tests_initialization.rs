use super::test_support::{expect_id, expect_method, read_existing_message};
use super::{IncomingMessage, LspClient};
use crate::lsp::transport::write_message;
use crate::test_support::TestDir;
use serde_json::{Value, json};
use std::io::BufReader;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::Duration;

struct SocketFixture {
    _dir: TestDir,
    socket_path: PathBuf,
    server: JoinHandle<()>,
}

impl SocketFixture {
    fn spawn(name: &str, run_server: impl FnOnce(Peer) + Send + 'static) -> Self {
        let dir = TestDir::new(name);
        let socket_path = dir.path().join("server.sock");
        let listener = UnixListener::bind(&socket_path).expect("socket should bind");
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("client should connect");
            run_server(Peer::new(stream));
        });
        Self {
            _dir: dir,
            socket_path,
            server,
        }
    }

    fn connect(&self) -> LspClient {
        LspClient::connect_unix(&self.socket_path, false, Duration::from_secs(1))
            .expect("client should connect")
    }

    fn finish(self) {
        self.server.join().expect("server thread should finish");
    }
}

struct Peer {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Peer {
    fn new(stream: UnixStream) -> Self {
        let reader = BufReader::new(stream.try_clone().expect("stream should clone"));
        Self {
            reader,
            writer: stream,
        }
    }

    fn read(&mut self, context: &str) -> Value {
        read_existing_message(
            &mut self.reader,
            &format!("{context} should parse"),
            &format!("{context} should exist"),
        )
    }

    fn initialize(&mut self) -> Value {
        let request = self.read("initialize");
        expect_method(&request, "initialize");
        self.respond(&request, &json!({ "capabilities": {} }), "initialize");
        request
    }

    fn expect_initialized(&mut self) {
        expect_method(&self.read("initialized"), "initialized");
    }

    fn respond(&mut self, request: &Value, result: &Value, context: &str) {
        write_message(
            &mut self.writer,
            &json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().expect("request id should exist"),
                "result": result,
            }),
        )
        .unwrap_or_else(|error| panic!("{context} response should write: {error}"));
    }

    fn send(&mut self, message: &Value, context: &str) {
        write_message(&mut self.writer, message)
            .unwrap_or_else(|error| panic!("{context} should write: {error}"));
    }

    fn finish_shutdown(&mut self) {
        let shutdown = self.read("shutdown");
        expect_method(&shutdown, "shutdown");
        self.respond(&shutdown, &Value::Null, "shutdown");
        expect_method(&self.read("exit"), "exit");
    }
}

fn initialize_client(client: &mut LspClient) {
    client
        .initialize("file:///workspace", "workspace", false)
        .expect("initialize should succeed");
}

#[test]
fn queued_server_request_is_answered_before_next_client_request() {
    let fixture = SocketFixture::spawn("client-init-queue", |mut peer| {
        peer.initialize();
        peer.expect_initialized();

        let registration_response = peer.read("registration response");
        expect_id(&registration_response, "register-1");
        assert_eq!(registration_response.get("result"), Some(&Value::Null));

        let symbols = peer.read("workspace symbol request");
        expect_method(&symbols, "workspace/symbol");
        peer.respond(&symbols, &json!([]), "workspace symbol");
        peer.finish_shutdown();
    });
    let mut client = fixture.connect();
    initialize_client(&mut client);
    client
        .pending_messages
        .push_back(IncomingMessage::Message(json!({
            "jsonrpc": "2.0",
            "id": "register-1",
            "method": "client/registerCapability",
            "params": { "registrations": [] },
        })));

    assert_eq!(
        client
            .workspace_symbol("needle")
            .expect("workspace symbol should succeed"),
        json!([])
    );
    client.shutdown().expect("shutdown should succeed");
    fixture.finish();
}

#[test]
fn server_request_is_answered_while_client_request_is_outstanding() {
    let fixture = SocketFixture::spawn("client-init-in-flight", |mut peer| {
        peer.initialize();
        peer.expect_initialized();

        let symbols = peer.read("workspace symbol request");
        expect_method(&symbols, "workspace/symbol");
        peer.send(
            &json!({
                "jsonrpc": "2.0",
                "id": "configuration-1",
                "method": "workspace/configuration",
                "params": { "items": [] },
            }),
            "configuration request",
        );
        let configuration_response = peer.read("configuration response");
        expect_id(&configuration_response, "configuration-1");
        assert_eq!(configuration_response.get("result"), Some(&json!([])));
        peer.respond(&symbols, &json!([]), "workspace symbol");
        peer.finish_shutdown();
    });
    let mut client = fixture.connect();
    initialize_client(&mut client);

    assert_eq!(
        client
            .workspace_symbol("needle")
            .expect("workspace symbol should succeed"),
        json!([])
    );
    client.shutdown().expect("shutdown should succeed");
    fixture.finish();
}

#[test]
fn initialize_advertises_and_returns_workspace_folders() {
    let fixture = SocketFixture::spawn("client-init-workspace-folders", |mut peer| {
        let initialize = peer.initialize();
        let params = initialize
            .get("params")
            .expect("initialize params should exist");
        assert_eq!(
            params
                .pointer("/capabilities/workspace/workspaceFolders")
                .and_then(Value::as_bool),
            Some(true)
        );
        let workspace_folders = params
            .get("workspaceFolders")
            .cloned()
            .expect("workspaceFolders should exist");
        peer.expect_initialized();

        let folders_response = peer.read("workspace folders response");
        expect_id(&folders_response, "folders-1");
        assert_eq!(folders_response.get("result"), Some(&workspace_folders));
        peer.finish_shutdown();
    });
    let mut client = fixture.connect();
    initialize_client(&mut client);
    client
        .pending_messages
        .push_back(IncomingMessage::Message(json!({
            "jsonrpc": "2.0",
            "id": "folders-1",
            "method": "workspace/workspaceFolders",
        })));

    client.shutdown().expect("shutdown should succeed");
    fixture.finish();
}
