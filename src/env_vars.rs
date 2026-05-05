use std::ffi::OsString;
use std::path::PathBuf;

/// User home directory used for per-user runtime state and default config locations.
pub(crate) const HOME: &str = "HOME";

/// Override for the active lsp-cli data tree.
pub(crate) const LSP_DATA: &str = "LSP_DATA";

/// Per-user runtime directory for daemon sockets.
pub(crate) const XDG_RUNTIME_DIR: &str = "XDG_RUNTIME_DIR";

/// Per-user config root used to locate `lsp-cli/lsp-cli.yaml`.
pub(crate) const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";

/// Executable search path used to locate runtimes and already-installed servers.
pub(crate) const PATH: &str = "PATH";

/// Current interactive shell used for shell auto-detection in completion output.
pub(crate) const SHELL: &str = "SHELL";

#[cfg(test)]
/// Test-only override that tells fake npm installs which executable to materialize.
pub(crate) const TEST_FAKE_NPM_PROGRAM: &str = "LSP_CLI_TEST_FAKE_NPM_PROGRAM";

fn path_var(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

// Q: do not add _dir to wrapper names
pub(crate) fn home_dir() -> Option<PathBuf> {
    path_var(HOME)
}

pub(crate) fn lsp_data_dir() -> Option<PathBuf> {
    path_var(LSP_DATA)
}

pub(crate) fn xdg_runtime_dir() -> Option<PathBuf> {
    path_var(XDG_RUNTIME_DIR)
}

pub(crate) fn xdg_config_home() -> Option<PathBuf> {
    path_var(XDG_CONFIG_HOME)
}

// Q: rename path_value -> path
pub(crate) fn path_value() -> Option<OsString> {
    std::env::var_os(PATH)
}

// Q: rename shell_path -> shell
pub(crate) fn shell_path() -> Option<OsString> {
    std::env::var_os(SHELL)
}

#[cfg(test)]
pub(crate) fn fake_npm_program() -> Option<OsString> {
    std::env::var_os(TEST_FAKE_NPM_PROGRAM)
}
