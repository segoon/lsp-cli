use super::LspClient;
use crate::test_support::{TestDir, env_var, runtime_state_in_home, with_env_vars};
use std::fs;
use std::time::Duration;

#[cfg(unix)]
#[test]
fn logs_server_start_stderr_and_exit_to_global_log() {
    let dir = TestDir::new("client-system-log");
    let home = dir.path().join("home");
    let workspace_root = dir.path().join("workspace");
    fs::create_dir_all(&home).expect("home should exist");
    fs::create_dir_all(&workspace_root).expect("workspace should exist");
    let log_path = runtime_state_in_home(&home).log_path();
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf 'clangd (1.2.3)\n' >&2; exit 1".to_string(),
    ];

    with_env_vars(&[env_var("HOME", &home)], || {
        let mut client = LspClient::new(&command, &workspace_root, false, Duration::from_secs(1))
            .expect("helper process should start");
        let _ = client
            .initialize("file:///workspace", "workspace", false)
            .expect_err("initialize should fail");
        drop(client);
    });

    let log = fs::read_to_string(log_path).expect("global log should be readable");
    assert!(log.contains("starting LSP server..."));
    assert!(log.contains("LSP server has started (pid "));
    assert!(log.contains("stderr: clangd (1.2.3)"));
    assert!(log.contains("LSP server exited with exit code 1"));
}
