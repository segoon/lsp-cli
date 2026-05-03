use super::{
    choose_cli_config_user_root, choose_config_root, default_config_root, load_cli_config,
    load_config_store,
};
use crate::test_support::{LOCAL_SHARE_LSP_CLI, TestDir};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

const EMPTY_FILETYPE_YAML: &str = "extensions: []\npatterns: []\n";
const MINIMAL_LSP_YAML: &str =
    "filetypes: []\nroot_markers: []\nname: placeholder\ncmdline: placeholder\n";

fn write_config_dirs(dir: &TestDir) {
    dir.write_file("filetypes/placeholder.yaml", EMPTY_FILETYPE_YAML);
    dir.write_file("lsp/placeholder.yaml", MINIMAL_LSP_YAML);
}

fn write_local_share_config(home: &TestDir, filetype: &str, lsp: &str) {
    home.write_file(
        &format!("{LOCAL_SHARE_LSP_CLI}/data/filetypes/{filetype}.yaml"),
        EMPTY_FILETYPE_YAML,
    );
    home.write_file(
        &format!("{LOCAL_SHARE_LSP_CLI}/data/lsp/{lsp}.yaml"),
        format!("filetypes: []\nroot_markers: []\nname: {lsp}\ncmdline: {lsp}\n"),
    );
}

#[test]
fn resolves_config_root_from_lsp_data_env() {
    let lsp_data = TestDir::new("config");
    let home = TestDir::new("config");
    let repo = TestDir::new("config");
    lsp_data.write_file("filetypes/a.yaml", EMPTY_FILETYPE_YAML);
    lsp_data.write_file(
        "lsp/a.yaml",
        "filetypes: []\nroot_markers: []\nname: a\ncmdline: a\n",
    );
    write_local_share_config(&home, "b", "b");
    write_config_dirs(&repo);

    assert_eq!(
        choose_config_root(Some(lsp_data.path()), Some(home.path()), repo.path())
            .expect("root should resolve"),
        lsp_data.path()
    );
}

#[test]
fn falls_back_to_home_local_share() {
    let home = TestDir::new("config");
    let repo = TestDir::new("config");
    write_local_share_config(&home, "c", "clangd");
    write_config_dirs(&repo);

    assert_eq!(
        choose_config_root(None, Some(home.path()), repo.path()).expect("root should resolve"),
        home.path().join(format!("{LOCAL_SHARE_LSP_CLI}/data"))
    );
}

#[test]
fn falls_back_to_repo_data_when_home_default_missing() {
    let home = TestDir::new("config");
    let repo = TestDir::new("config");
    write_config_dirs(&repo);

    assert_eq!(
        choose_config_root(None, Some(home.path()), repo.path()).expect("root should resolve"),
        repo.path()
    );
}

#[test]
fn errors_when_no_root_can_be_resolved() {
    let home = TestDir::new("config");
    let repo = TestDir::new("config");

    let error = choose_config_root(None, Some(home.path()), repo.path())
        .expect_err("root resolution should fail");

    assert!(error.contains("could not resolve config root"));
}

#[test]
fn default_config_root_resolves_in_real_environment() {
    let root = default_config_root().expect("root should resolve");

    assert!(root.ends_with(".local/share/lsp-cli/data") || root.ends_with("data"));
}

#[test]
fn resolves_cli_user_root_from_xdg_config_home() {
    let xdg_config_home = TestDir::new("cli-config-root");
    let home = TestDir::new("cli-config-root");

    assert_eq!(
        choose_cli_config_user_root(Some(xdg_config_home.path()), Some(home.path())),
        Some(xdg_config_home.path().join("lsp-cli"))
    );
}

#[test]
fn falls_back_to_home_dot_config_for_cli_user_root() {
    let home = TestDir::new("cli-config-root");

    assert_eq!(
        choose_cli_config_user_root(None, Some(home.path())),
        Some(home.path().join(".config/lsp-cli"))
    );
}

#[test]
fn returns_no_cli_user_root_without_xdg_config_home_or_home() {
    assert_eq!(choose_cli_config_user_root(None, None), None);
}

#[test]
fn loads_valid_config_store() {
    let dir = TestDir::new("config");
    dir.write_file(
        "filetypes/c.yaml",
        "extensions:\n  - c\n  - h\npatterns:\n  - '^special$'\n",
    );
    dir.write_file("filetypes/cpp.yaml", "extensions:\n  - cpp\npatterns: []\n");
    dir.write_file(
        "lsp/clangd.yaml",
        concat!(
            "filetypes:\n",
            "  - c\n",
            "  - cpp\n",
            "root_markers:\n",
            "  - compile_commands.json\n",
            "name: clangd\n",
            "cmdline: clangd --background-index $WORKSPACE\n",
            "wait-for-index: true\n"
        ),
    );

    let config = load_config_store(dir.path()).expect("config should load");

    assert_eq!(config.filetypes.len(), 2);
    assert_eq!(config.lsps.len(), 1);
    assert!(config.filetypes.iter().any(|filetype| filetype.id == "c"));
    assert_eq!(config.lsps[0].id, "clangd");
    assert_eq!(config.lsps[0].name, "clangd");
    assert!(config.lsps[0].wait_for_index);
    assert_eq!(config.cli, super::CliConfig::default());
}

#[test]
fn loads_layered_cli_config_with_user_overrides() {
    let global = TestDir::new("cli-config-global");
    let user = TestDir::new("cli-config-user");
    global.write_file(
        "lsp-cli.yaml",
        concat!(
            "download: true\n",
            "detach: true\n",
            "timeout: 1.5\n",
            "limit: 20\n",
            "detect:\n",
            "  quiet: true\n",
            "daemon:\n",
            "  idle-timeout: 5\n",
            "lsp:\n",
            "  cpp:\n",
            "    - clangd\n",
            "  python:\n",
            "    - pyright\n"
        ),
    );
    user.write_file(
        "lsp-cli.yaml",
        concat!(
            "json: true\n",
            "debug: true\n",
            "limit: 50\n",
            "daemon:\n",
            "  idle-timeout: 10\n",
            "lsp:\n",
            "  python:\n",
            "    - ty\n",
            "    - pyright\n"
        ),
    );

    let config = load_cli_config(global.path(), Some(user.path())).expect("cli config should load");

    assert_eq!(config.download_version, None);
    assert_eq!(config.download, Some(true));
    assert_eq!(config.detach, Some(true));
    assert_eq!(config.json, Some(true));
    assert_eq!(config.debug, Some(true));
    assert_eq!(config.timeout, Some(std::time::Duration::from_millis(1500)));
    assert_eq!(config.limit, Some(50));
    assert_eq!(config.detect.quiet, Some(true));
    assert_eq!(
        config.daemon.idle_timeout,
        Some(std::time::Duration::from_secs(10))
    );
    assert_eq!(
        config.lsp_preferences,
        BTreeMap::from([
            ("cpp".to_string(), vec!["clangd".to_string()]),
            (
                "python".to_string(),
                vec!["ty".to_string(), "pyright".to_string()],
            ),
        ])
    );
}

#[test]
fn ignores_missing_cli_config_files() {
    let global = TestDir::new("cli-config-global-missing");
    let user = TestDir::new("cli-config-user-missing");

    let config = load_cli_config(global.path(), Some(user.path()))
        .expect("missing cli config should be ignored");

    assert_eq!(config, super::CliConfig::default());
}

#[test]
fn loads_download_version_with_user_override() {
    let global = TestDir::new("cli-config-download-version-global");
    let user = TestDir::new("cli-config-download-version-user");
    global.write_file("lsp-cli.yaml", "download-version: stable\n");
    user.write_file("lsp-cli.yaml", "download-version: latest\n");

    let config = load_cli_config(global.path(), Some(user.path())).expect("cli config should load");

    assert_eq!(config.download_version.as_deref(), Some("latest"));
}

#[test]
fn fails_on_invalid_cli_config() {
    let global = TestDir::new("cli-config-invalid");
    global.write_file("lsp-cli.yaml", "timeout: nope\n");

    let error = load_cli_config(global.path(), None).expect_err("invalid cli config should fail");

    assert!(error.contains("lsp-cli.yaml"));
    assert!(error.contains("invalid timeout"));
}

#[test]
fn rejects_unknown_cli_config_keys() {
    let global = TestDir::new("cli-config-unknown");
    global.write_file("lsp-cli.yaml", "lang: cpp\n");

    let error = load_cli_config(global.path(), None).expect_err("unknown keys should fail");

    assert!(error.contains("unknown field `lang`"));
}

#[test]
fn fails_when_config_root_is_missing() {
    let missing = std::env::temp_dir().join(format!(
        "lsp-cli-config-missing-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos()
    ));

    let error = load_config_store(&missing).expect_err("config load should fail");

    assert!(error.contains("missing directory"));
}

#[test]
fn fails_on_invalid_yaml() {
    let dir = TestDir::new("config");
    dir.write_file("filetypes/c.yaml", "extensions: [c\n");
    dir.write_file(
        "lsp/clangd.yaml",
        "filetypes: [c]\nroot_markers: []\nname: clangd\ncmdline: clangd\n",
    );

    let error = load_config_store(dir.path()).expect_err("config load should fail");

    assert!(error.contains("filetypes/c.yaml"));
}

#[test]
fn fails_on_unknown_lsp_filetype() {
    let dir = TestDir::new("config");
    dir.write_file("filetypes/c.yaml", "extensions: [c]\npatterns: []\n");
    dir.write_file(
        "lsp/clangd.yaml",
        "filetypes: [cpp]\nroot_markers: []\nname: clangd\ncmdline: clangd\n",
    );

    let error = load_config_store(dir.path()).expect_err("config load should fail");

    assert!(error.contains("unknown filetype cpp"));
}

#[test]
fn fails_on_invalid_regex() {
    let dir = TestDir::new("config");
    dir.write_file("filetypes/c.yaml", "extensions: [c]\npatterns: ['(']\n");
    dir.write_file(
        "lsp/clangd.yaml",
        "filetypes: [c]\nroot_markers: []\nname: clangd\ncmdline: clangd\n",
    );

    let error = load_config_store(dir.path()).expect_err("config load should fail");

    assert!(error.contains("invalid regex"));
}
