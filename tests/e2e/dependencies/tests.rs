use std::io::Write as _;
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt as _;
use std::thread;

use super::*;
use crate::dependencies::catalog::Validation;

const PAYLOAD_SHA256: &str = "672eb8316fec83f94119a4193f9fc552513d56a147502f8be4830e017d817831";

#[test]
fn parses_auto_download_policy_without_mutating_process_environment() {
    assert!(auto_download(Err(std::env::VarError::NotPresent)).expect("default should enable"));
    assert!(auto_download(Ok("1".to_string())).expect("one should enable"));
    assert!(!auto_download(Ok("0".to_string())).expect("zero should disable"));
    assert_eq!(
        auto_download(Ok("sometimes".to_string())).expect_err("other values should fail"),
        "E2E_AUTO_DOWNLOAD must be 0 or 1"
    );
}

fn dependency(url: String) -> Dependency {
    Dependency {
        name: "fixture",
        version: "1.0.0",
        platform: "test-platform".to_string(),
        url,
        archive_name: "fixture.tar.gz",
        digest: Digest::Sha256(PAYLOAD_SHA256),
        strip_components: 1,
        path_directory: "bin",
        validation: Validation {
            program: "bin/fixture",
            args: &["--version"],
            expected: "1.0.0",
            include_stderr: false,
        },
    }
}

fn serve(responses: Vec<(u16, &'static [u8])>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture server should bind");
    let address = listener
        .local_addr()
        .expect("fixture server address should resolve");
    let server = thread::spawn(move || {
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().expect("fixture request should arrive");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let read = stream
                    .read(&mut buffer)
                    .expect("fixture request should be readable");
                assert!(read > 0, "fixture request ended before its headers");
                request.extend_from_slice(
                    buffer
                        .get(..read)
                        .expect("read byte count should fit the fixture buffer"),
                );
            }
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .and_then(|()| stream.write_all(body))
                .expect("fixture response should be written");
        }
    });
    (format!("http://{address}/fixture"), server)
}

fn test_cache(path: &Path) -> Cache {
    Cache {
        root: path.to_path_buf(),
        client: Client::builder()
            .no_proxy()
            .pool_max_idle_per_host(0)
            .build()
            .expect("fixture HTTP client should build"),
    }
}

#[test]
fn hashes_cached_downloads() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let path = directory.path().join("payload");
    fs::write(&path, b"verified\n").expect("payload should be written");
    assert!(verify_digest(&path, Digest::Sha256(PAYLOAD_SHA256)).expect("payload should hash"));
}

#[test]
fn download_cache_avoids_a_second_request() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let (url, server) = serve(vec![(200, b"verified\n")]);
    let dependency = dependency(url);
    let cache = test_cache(&directory.path().join("cache"));

    let first = cache
        .ensure_download(&dependency)
        .expect("first download should succeed");
    server.join().expect("fixture server should finish");
    let second = cache
        .ensure_download(&dependency)
        .expect("cached download should succeed without its server");

    assert_eq!(first, second);
}

#[test]
fn retries_transient_download_failures() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let (url, server) = serve(vec![(500, b""), (503, b""), (200, b"verified\n")]);
    let dependency = dependency(url);
    let cache = test_cache(&directory.path().join("cache"));

    cache
        .ensure_download(&dependency)
        .expect("third download attempt should succeed");
    server.join().expect("fixture server should finish");
}

#[test]
fn invalid_completed_installation_is_preserved() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let dependency = dependency("http://127.0.0.1:1/unreachable".to_string());
    let cache = Cache::new(directory.path().join("cache"));
    let installation = cache.installation_path(&dependency);
    fs::create_dir_all(&installation).expect("invalid installation should be created");

    let error = cache
        .ensure(&dependency)
        .expect_err("invalid completed installation should fail");

    assert!(installation.is_dir());
    assert!(error.contains("make clean-e2e-dependencies"));
}

#[test]
fn extracts_archives_with_the_declared_strip_depth() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let source = directory.path().join("source/root/bin");
    fs::create_dir_all(&source).expect("archive fixture directory should be created");
    let program = source.join("fixture");
    fs::write(&program, "#!/bin/sh\necho 1.0.0\n").expect("fixture program should be written");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755))
        .expect("fixture program should be executable");
    let archive = directory.path().join("fixture.tar.gz");
    let status = Command::new("tar")
        .args(["-czf"])
        .arg(&archive)
        .arg("-C")
        .arg(directory.path().join("source"))
        .arg("root")
        .status()
        .expect("tar should run");
    assert!(status.success());
    let destination = directory.path().join("destination");
    fs::create_dir(&destination).expect("extraction destination should be created");

    extract(
        &dependency("http://unused".to_string()),
        &archive,
        &destination,
    )
    .expect("fixture should extract");

    assert!(destination.join("bin/fixture").is_file());
}

#[test]
fn assembles_managed_directories_before_host_path() {
    let dependencies = catalog::dependencies().expect("catalog should load");
    let installations = vec![
        (&dependencies[0], PathBuf::from("/managed/go")),
        (&dependencies[3], PathBuf::from("/managed/dotnet")),
    ];
    let host = std::env::join_paths(["/host/one", "/host/two"]).expect("host PATH should join");
    let actual = assemble_install_path(&installations, &host).expect("managed PATH should join");

    assert_eq!(
        std::env::split_paths(&actual).collect::<Vec<_>>(),
        [
            PathBuf::from("/managed/go/bin"),
            PathBuf::from("/managed/dotnet"),
            PathBuf::from("/host/one"),
            PathBuf::from("/host/two"),
        ]
    );
}
