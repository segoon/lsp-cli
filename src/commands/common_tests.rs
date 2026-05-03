use super::{PreparedWorkspace, connect_lsp_client, prepare_workspace, resolve_server};
use crate::config::load_config_store;
use crate::lsp::transport::{read_message, write_message};
use crate::suggest::SuggestedLanguage;
use crate::test_support::{
    SUBPROCESS_HELPER_EXIT_CODE_ENV, SUBPROCESS_HELPER_OUTPUT_PATH_ENV, TestDir,
    current_test_executable, detection_result, env_var, make_executable, pyright_package,
    runtime_state_in_home, subprocess_helper_command, subprocess_helper_env, with_env_vars,
    without_env_vars, write_registry,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::BufReader;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::thread;
use std::time::Duration;

fn server(
    config_id: &str,
    server: &str,
    languages: &[&str],
    command: &[&str],
) -> SuggestedLanguage {
    server_with_command(
        config_id,
        server,
        languages,
        command.iter().map(|part| (*part).to_string()).collect(),
    )
}

fn server_with_command(
    config_id: &str,
    server: &str,
    languages: &[&str],
    command: Vec<String>,
) -> SuggestedLanguage {
    SuggestedLanguage {
        config_id: config_id.to_string(),
        languages: languages
            .iter()
            .map(|language| (*language).to_string())
            .collect(),
        server: server.to_string(),
        command,
        workspace_root: PathBuf::from("."),
        wait_for_index: false,
    }
}

fn example_suggestion() -> SuggestedLanguage {
    server_with_command(
        "example_lsp",
        "example-lsp",
        &["alpha", "beta"],
        vec![current_test_executable().display().to_string()],
    )
}

fn daemon_workspace(
    workspace_root: &Path,
    command: Vec<String>,
    socket_path: Option<PathBuf>,
) -> PreparedWorkspace {
    PreparedWorkspace {
        detection: detection_result(&["rust"], &[]),
        server: SuggestedLanguage {
            config_id: "rust-analyzer".to_string(),
            languages: vec!["rust".to_string()],
            server: "rust-analyzer".to_string(),
            command,
            workspace_root: workspace_root.to_path_buf(),
            wait_for_index: false,
        },
        allowed_filetypes: BTreeSet::from(["rust".to_string()]),
        root_uri: crate::lsp::path_to_file_uri(workspace_root).expect("root uri should build"),
        workspace_name: crate::lsp::workspace_name(workspace_root),
        daemon_socket_path: socket_path,
        daemon_socket_error: None,
    }
}

#[test]
fn selects_requested_server_for_grep() {
    let primary = example_suggestion();
    let secondary = server_with_command(
        "secondary_lsp",
        "secondary-lsp",
        &["beta"],
        vec![current_test_executable().display().to_string()],
    );
    let suggestions = [primary, secondary.clone()];

    let selected = resolve_server(
        &detection_result(&["beta"], &[]),
        &suggestions,
        Some("secondary-lsp"),
        None,
        &BTreeMap::new(),
        false,
    )
    .expect("requested server should be selected");

    assert_eq!(selected.server.server, secondary.server);
}

#[test]
fn errors_when_requested_server_is_not_detected() {
    let error = resolve_server(
        &detection_result(&["beta"], &[]),
        &[example_suggestion()],
        Some("missing-lsp"),
        None,
        &BTreeMap::new(),
        false,
    )
    .expect_err("missing server should error");

    assert_eq!(
        error,
        "requested LSP server \"missing-lsp\" is not in the detected server list: example-lsp"
    );
}

#[cfg(unix)]
#[test]
fn resolves_server_from_managed_install() {
    let dir = TestDir::new("common");
    let home = dir.path().join("home");
    let state = runtime_state_in_home(&home);
    state.ensure_dirs().expect("state dirs should be created");
    write_registry(&state, &[pyright_package()]);
    let cached = state
        .package_dir("pyright")
        .join("node_modules/.bin/pyright-langserver");
    fs::create_dir_all(cached.parent().expect("parent should exist"))
        .expect("parent dirs should be created");
    fs::write(&cached, b"stub\n").expect("cached binary should be written");
    make_executable(&cached);

    let resolved = with_env_vars(
        &[env_var("HOME", &home), env_var("PATH", "/nonexistent")],
        || {
            resolve_server(
                &detection_result(&["python"], &[]),
                &[server(
                    "pyright",
                    "pyright-langserver",
                    &["python"],
                    &["pyright-langserver", "--stdio"],
                )],
                None,
                None,
                &BTreeMap::new(),
                false,
            )
            .expect("server should resolve")
        },
    );

    assert_eq!(resolved.server.command[0], cached.display().to_string());
}

#[test]
fn errors_when_auto_selection_spans_multiple_languages() {
    let error = resolve_server(
        &detection_result(&["alpha", "beta"], &[]),
        &[example_suggestion()],
        None,
        None,
        &BTreeMap::new(),
        false,
    )
    .expect_err("multiple languages should require disambiguation");

    assert_eq!(
        error,
        "multiple languages were detected for this command: alpha, beta; pass --lang LANG or --lsp SERVER to choose one"
    );
}

#[test]
fn allows_auto_selection_with_explicit_language() {
    let resolved = resolve_server(
        &detection_result(&["alpha", "beta"], &[]),
        &[example_suggestion()],
        None,
        Some("beta"),
        &BTreeMap::new(),
        false,
    )
    .expect("language should disambiguate");

    assert_eq!(
        resolved.allowed_filetypes,
        BTreeSet::from(["beta".to_string()])
    );
}

#[cfg(unix)]
#[test]
fn skips_unrunnable_servers_without_download() {
    let dir = TestDir::new("common-select-installed");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).expect("home dir should be created");
    let bin_dir = dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir should be created");
    let fallback = bin_dir.join("fallback-lsp");
    fs::write(&fallback, b"stub\n").expect("fallback server should be written");
    make_executable(&fallback);

    let resolved = with_env_vars(
        &[
            env_var("HOME", &home),
            env_var("PATH", bin_dir.display().to_string()),
        ],
        || {
            resolve_server(
                &detection_result(&["python"], &[]),
                &[
                    server(
                        "pyright",
                        "pyright",
                        &["python"],
                        &["pyright-langserver", "--stdio"],
                    ),
                    server(
                        "fallback",
                        "fallback-lsp",
                        &["python"],
                        &["fallback-lsp", "--stdio"],
                    ),
                ],
                None,
                None,
                &BTreeMap::from([(
                    "python".to_string(),
                    vec!["pyright".to_string(), "fallback-lsp".to_string()],
                )]),
                false,
            )
            .expect("fallback server should be selected")
        },
    );

    assert_eq!(resolved.server.server, "fallback-lsp");
    assert_eq!(resolved.server.command[0], "fallback-lsp");
}

#[cfg(unix)]
#[test]
fn downloads_selected_server_when_requested() {
    let dir = TestDir::new("common-download-selected");
    let home = dir.path().join("home");
    let state = runtime_state_in_home(&home);
    state.ensure_dirs().expect("state dirs should be created");
    write_registry(&state, &[pyright_package()]);

    let bin_dir = dir.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("bin dir should be created");
    let fallback = bin_dir.join("fallback-lsp");
    fs::write(&fallback, b"stub\n").expect("fallback server should be written");
    make_executable(&fallback);
    let npm = bin_dir.join("npm");
    fs::write(&npm, b"stub\n").expect("fake npm should be written");
    make_executable(&npm);

    let envs = vec![
        env_var("HOME", &home),
        env_var("PATH", bin_dir.display().to_string()),
        env_var("LSP_CLI_TEST_FAKE_NPM_PROGRAM", "pyright-langserver"),
    ];

    let resolved = with_env_vars(&envs, || {
        resolve_server(
            &detection_result(&["python"], &[]),
            &[
                server(
                    "pyright",
                    "pyright",
                    &["python"],
                    &["pyright-langserver", "--stdio"],
                ),
                server(
                    "fallback",
                    "fallback-lsp",
                    &["python"],
                    &["fallback-lsp", "--stdio"],
                ),
            ],
            None,
            None,
            &BTreeMap::from([(
                "python".to_string(),
                vec!["pyright".to_string(), "fallback-lsp".to_string()],
            )]),
            true,
        )
        .expect("preferred server should install and resolve")
    });

    let installed = state
        .package_dir("pyright")
        .join("node_modules/.bin/pyright-langserver");
    assert_eq!(resolved.server.server, "pyright");
    assert_eq!(resolved.server.command[0], installed.display().to_string());
    assert!(installed.exists(), "preferred server should be installed");
}

#[cfg(unix)]
#[test]
fn prefers_live_daemon_socket_before_spawning_server() {
    let dir = TestDir::new("common-daemon");
    let runtime_dir = dir.path().join("runtime");
    fs::create_dir_all(runtime_dir.join("lsp-cli")).expect("runtime dir should be created");
    let socket_path = runtime_dir.join("lsp-cli/test.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");
    let cwd_file = dir.path().join("cwd.txt");
    let workspace_root = dir.path().join("workspace");
    fs::create_dir_all(&workspace_root).expect("workspace should exist");
    let command = subprocess_helper_command();

    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("client should connect");
        let reader_stream = stream.try_clone().expect("socket should clone");
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;

        let initialize = read_message(&mut reader)
            .expect("initialize should parse")
            .expect("initialize should exist");
        assert_eq!(
            initialize.get("method").and_then(serde_json::Value::as_str),
            Some("initialize")
        );
        write_message(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": initialize.get("id").cloned().expect("initialize id should exist"),
                "result": { "capabilities": {} },
            }),
        )
        .expect("initialize response should write");

        let initialized = read_message(&mut reader)
            .expect("initialized should parse")
            .expect("initialized should exist");
        assert_eq!(
            initialized
                .get("method")
                .and_then(serde_json::Value::as_str),
            Some("initialized")
        );

        let shutdown = read_message(&mut reader)
            .expect("shutdown should parse")
            .expect("shutdown should exist");
        assert_eq!(
            shutdown.get("method").and_then(serde_json::Value::as_str),
            Some("shutdown")
        );
        write_message(
            &mut writer,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": shutdown.get("id").cloned().expect("shutdown id should exist"),
                "result": null,
            }),
        )
        .expect("shutdown response should write");

        let exit = read_message(&mut reader)
            .expect("exit should parse")
            .expect("exit should exist");
        assert_eq!(
            exit.get("method").and_then(serde_json::Value::as_str),
            Some("exit")
        );
    });

    let mut envs = vec![env_var("XDG_RUNTIME_DIR", &runtime_dir)];
    envs.extend(subprocess_helper_env(
        "write-cwd",
        &[env_var(SUBPROCESS_HELPER_OUTPUT_PATH_ENV, &cwd_file)],
    ));

    with_env_vars(&envs, || {
        let mut client = connect_lsp_client(
            &daemon_workspace(&workspace_root, command.clone(), Some(socket_path.clone())),
            false,
            false,
            Duration::from_secs(1),
        )
        .expect("client should connect");

        client
            .initialize(
                &crate::lsp::path_to_file_uri(&workspace_root).expect("root uri should build"),
                &crate::lsp::workspace_name(&workspace_root),
                false,
            )
            .expect("initialize should succeed");
        client.shutdown().expect("shutdown should succeed");
    });

    server.join().expect("daemon thread should finish");
    assert!(
        !cwd_file.exists(),
        "direct server should not have been spawned"
    );
}

#[cfg(unix)]
#[test]
fn removes_dead_daemon_socket_and_falls_back_to_server_process() {
    let dir = TestDir::new("common-dead-daemon");
    let runtime_dir = dir.path().join("runtime");
    fs::create_dir_all(runtime_dir.join("lsp-cli")).expect("runtime dir should be created");
    let socket_path = runtime_dir.join("lsp-cli/test.sock");
    let listener = UnixListener::bind(&socket_path).expect("socket should bind");
    drop(listener);

    let workspace_root = dir.path().join("workspace");
    fs::create_dir_all(&workspace_root).expect("workspace should exist");
    let command = subprocess_helper_command();

    let mut envs = vec![env_var("XDG_RUNTIME_DIR", &runtime_dir)];
    envs.extend(subprocess_helper_env(
        "stderr-and-exit",
        &[env_var(SUBPROCESS_HELPER_EXIT_CODE_ENV, "0")],
    ));

    with_env_vars(&envs, || {
        let _client = connect_lsp_client(
            &daemon_workspace(&workspace_root, command.clone(), Some(socket_path.clone())),
            false,
            false,
            Duration::from_secs(1),
        )
        .expect("client should fall back to process");
    });

    assert!(!socket_path.exists(), "dead socket should be removed");
}

#[test]
fn preserves_daemon_root_error_for_strict_detach_mode() {
    let dir = TestDir::new("common-daemon-root-error");
    let workspace_root = dir.path().join("workspace");
    fs::create_dir_all(workspace_root.join("src")).expect("workspace should exist");
    fs::write(
        workspace_root.join("Cargo.toml"),
        b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("cargo manifest should be written");
    fs::write(workspace_root.join("src/main.rs"), b"fn main() {}\n")
        .expect("rust source should be written");

    let config =
        load_config_store(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"))
            .expect("repo config should load");
    let workspace = without_env_vars(&["XDG_RUNTIME_DIR"], || {
        prepare_workspace(&workspace_root, None, None, false, &config)
            .expect("workspace should still prepare")
    });

    assert!(workspace.daemon_socket_path.is_none());
    assert_eq!(
        workspace.daemon_socket_error.as_deref(),
        Some("could not resolve daemon socket root because $XDG_RUNTIME_DIR is not set")
    );
    let Err(error) = connect_lsp_client(&workspace, true, false, Duration::from_secs(1)) else {
        panic!("strict detach should fail");
    };

    assert_eq!(
        error,
        "cannot use --detach because could not resolve daemon socket root because $XDG_RUNTIME_DIR is not set"
    );
}
