use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::json;

use crate::fixture::E2eFixture;
use crate::manifest::CommandStrategy;

#[test]
fn update_command_path_uses_local_release_fixture() {
    let fixture = E2eFixture::new().expect("E2E fixture should initialize");
    assert_eq!(
        fixture
            .commands_for(CommandStrategy::UpdateFixture)
            .collect::<Vec<_>>(),
        ["update"]
    );
    let archive = data_archive();
    let server = HttpFixture::start(archive);
    let context = fixture.context();

    let output = context.run_with_env(
        &["update"],
        &[("LSP_CLI_DATA_RELEASE_API_URL", server.release_url())],
    );
    output.assert_success();
    assert_eq!(output.stdout_text(), "updated lsp-cli data to e2e-v1\n");
    assert!(
        context
            .installed_data()
            .join("filetypes/fake.yaml")
            .is_file()
    );
    server.finish();
}

struct HttpFixture {
    release_url: String,
    thread: Option<thread::JoinHandle<Result<(), String>>>,
}

impl HttpFixture {
    fn start(archive: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("HTTP fixture should bind");
        listener
            .set_nonblocking(true)
            .expect("HTTP fixture should become nonblocking");
        let address = listener
            .local_addr()
            .expect("HTTP fixture should have an address");
        let base = format!("http://{address}");
        let release_url = format!("{base}/release");
        let thread = thread::spawn(move || serve(listener, &base, &archive));
        Self {
            release_url,
            thread: Some(thread),
        }
    }

    fn release_url(&self) -> &str {
        &self.release_url
    }

    fn finish(mut self) {
        self.thread
            .take()
            .expect("HTTP fixture thread should be present")
            .join()
            .expect("HTTP fixture thread should not panic")
            .expect("HTTP fixture should serve both requests");
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        // Join here because an assertion may unwind before `finish`, leaving a fixture thread alive.
        if let Some(thread) = self.thread.take() {
            let _result = thread.join();
        }
    }
}

fn serve(listener: TcpListener, base: &str, archive: &[u8]) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut served = 0;
    while served < 2 && Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _address)) => {
                serve_request(&mut stream, base, archive)?;
                served += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(format!("HTTP fixture accept failed: {error}")),
        }
    }
    if served == 2 {
        Ok(())
    } else {
        Err(format!(
            "HTTP fixture served {served} of 2 expected requests"
        ))
    }
}

fn serve_request(stream: &mut TcpStream, base: &str, archive: &[u8]) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    let mut request = [0; 4096];
    let length = stream
        .read(&mut request)
        .map_err(|error| error.to_string())?;
    let request = String::from_utf8_lossy(
        request
            .get(..length)
            .expect("read byte count should fit its buffer"),
    );
    let (content_type, body) = if request.starts_with("GET /release ") {
        let body = json!({
            "tag_name": "e2e-v1",
            "tarball_url": format!("{base}/archive"),
            "zipball_url": null
        })
        .to_string()
        .into_bytes();
        ("application/json", body)
    } else if request.starts_with("GET /archive ") {
        ("application/gzip", archive.to_vec())
    } else {
        return Err(format!("unexpected HTTP fixture request: {request}"));
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .and_then(|()| stream.write_all(&body))
    .map_err(|error| error.to_string())
}

fn data_archive() -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut archive = tar::Builder::new(encoder);
    append(
        &mut archive,
        "data/filetypes/fake.yaml",
        b"extensions: [fake]\n",
    );
    append(
        &mut archive,
        "data/lsp/fake.yaml",
        b"filetypes: [fake]\nroot_markers: []\nname: fake\ncmdline: fake\n",
    );
    archive
        .into_inner()
        .expect("archive should finish")
        .finish()
        .expect("gzip stream should finish")
}

fn append(archive: &mut tar::Builder<GzEncoder<Vec<u8>>>, path: &str, contents: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(u64::try_from(contents.len()).expect("fixture size should fit u64"));
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, path, contents)
        .expect("fixture archive entry should append");
}
