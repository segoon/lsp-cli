use super::{ClientTransport, LspClient, format_spawn_error};
use crate::test_support::TestDir;
use std::fs;
use std::time::Duration;

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
