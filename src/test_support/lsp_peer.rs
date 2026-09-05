use super::TestDir;
use crate::lsp::LspClient;
use crate::lsp::transport::{read_message, write_message};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::BufReader;
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) struct LspPeer {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    opened: BTreeSet<String>,
}

impl LspPeer {
    pub(crate) fn spawn(
        dir: &TestDir,
        timeout: Duration,
        serve: impl FnOnce(&mut Self) + Send + 'static,
    ) -> (LspClient, JoinHandle<()>) {
        let path = dir.path().join("peer.sock");
        let listener = UnixListener::bind(&path).expect("bind fake server");
        let server = thread::spawn(move || {
            let (writer, _) = listener.accept().expect("accept client");
            writer
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("set timeout");
            writer
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("set timeout");
            let reader = BufReader::new(writer.try_clone().expect("clone socket"));
            serve(&mut Self {
                reader,
                writer,
                opened: BTreeSet::new(),
            });
        });
        (
            LspClient::connect_unix(&path, false, timeout).expect("connect client"),
            server,
        )
    }

    pub(crate) fn read(&mut self) -> Value {
        read_message(&mut self.reader)
            .expect("read message")
            .expect("message exists")
    }

    pub(crate) fn send(&mut self, message: &Value) {
        write_message(&mut self.writer, message).expect("send message");
    }

    pub(crate) fn reply(&mut self, request: &Value, result: Value) {
        let mut response = json!({"jsonrpc":"2.0", "id":request["id"]});
        response["result"] = result;
        self.send(&response);
    }

    pub(crate) fn document_request(&mut self) -> Value {
        loop {
            let message = self.read();
            match message["method"].as_str() {
                Some("textDocument/didOpen") => {
                    self.opened.insert(
                        message["params"]["textDocument"]["uri"]
                            .as_str()
                            .expect("uri")
                            .into(),
                    );
                }
                Some("textDocument/documentSymbol") => {
                    assert!(
                        self.opened.contains(
                            message["params"]["textDocument"]["uri"]
                                .as_str()
                                .expect("uri")
                        )
                    );
                    return message;
                }
                _ => panic!("unexpected message: {message}"),
            }
        }
    }

    pub(crate) fn finish(&mut self) {
        let shutdown = self.read();
        assert_eq!(shutdown["method"], "shutdown");
        self.reply(&shutdown, Value::Null);
        assert_eq!(self.read()["method"], "exit");
    }
}
