use super::LspClient;
use crate::test_support::TestDir;
use std::fs;
use std::time::Duration;

#[cfg(unix)]
#[test]
fn reports_server_stderr_when_initialize_closes_early() {
    let dir = TestDir::new("client-init-stderr");
    let workspace_root = dir.path().join("workspace");
    fs::create_dir_all(&workspace_root).expect("workspace should be created");
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf 'No ast-grep project configuration is found.\n' >&2; exit 1".to_string(),
    ];

    let mut client = LspClient::new(&command, &workspace_root, false, Duration::from_secs(1))
        .expect("helper process should start");
    let error = client
        .initialize("file:///workspace", "workspace", false)
        .expect_err("initialize should fail");

    assert!(error.contains("initialize"));
    assert!(error.contains("No ast-grep project configuration is found."));
}
