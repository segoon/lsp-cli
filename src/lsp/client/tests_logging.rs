use crate::server_stderr::CapturedStderr;
use crate::system_log::{
    log_lsp_server_cmdline, log_lsp_server_cwd, log_lsp_server_exit, log_lsp_server_started,
    log_lsp_server_starting,
};
use crate::test_support::{TestDir, env_var, runtime_state_in_home, with_env_vars};
use std::io::Cursor;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

#[cfg(unix)]
#[test]
fn logs_server_start_stderr_and_exit_to_global_log() {
    let dir = TestDir::new("client-system-log");
    let home = dir.path().join("home");
    let workspace_root = dir.path().join("workspace");
    std::fs::create_dir_all(&home).expect("home should exist");
    std::fs::create_dir_all(&workspace_root).expect("workspace should exist");
    let log_path = runtime_state_in_home(&home).log_path();
    let command = vec![
        workspace_root.join("helper").display().to_string(),
        "--stdio".to_string(),
    ];

    with_env_vars(&[env_var("HOME", &home)], || {
        log_lsp_server_starting();
        log_lsp_server_cmdline(&command);
        log_lsp_server_cwd(&workspace_root);
        log_lsp_server_started(1234);
        let stderr = CapturedStderr::spawn(Cursor::new(b"clangd (1.2.3)\n".to_vec()), false);
        assert_eq!(stderr.summary().as_deref(), Some("clangd (1.2.3)"));
        log_lsp_server_exit(ExitStatus::from_raw(1 << 8));
    });

    let log = std::fs::read_to_string(log_path).expect("global log should be readable");
    assert!(log.contains("starting LSP server..."));
    assert!(log.contains(&format!(
        "LSP server cmdline: {} --stdio",
        workspace_root.join("helper").display()
    )));
    assert!(log.contains(&format!("LSP server cwd: {}", workspace_root.display())));
    assert!(log.contains("LSP server has started (pid 1234)"));
    assert!(log.contains("stderr: clangd (1.2.3)"));
    assert!(log.contains("LSP server exited with exit code 1"));
}
