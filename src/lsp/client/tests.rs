use super::{ClientTransport, LspClient, format_spawn_error};
use crate::test_support::TestDir;
use std::fs;
use std::time::Duration;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::sync::{Mutex, OnceLock};

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
fn captured_server_stderr(debug: bool) -> String {
    let _lock = stderr_lock().lock().expect("stderr lock should be available");
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
