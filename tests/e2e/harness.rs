use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::TempDir;

#[path = "../../src/test_support/temp_root.rs"]
mod temp_root;

use self::temp_root::{test_temp_base, test_temp_root};

pub(crate) struct E2eContext {
    _sandbox: TempDir,
    _runtime_sandbox: TempDir,
    home: PathBuf,
    config_home: PathBuf,
    runtime_dir: PathBuf,
    workspace: PathBuf,
    bin_dir: PathBuf,
    data_dir: PathBuf,
}

impl E2eContext {
    pub(crate) fn new() -> io::Result<Self> {
        let test_temp_root = test_temp_root()?;
        fs::create_dir_all(&test_temp_root)?;
        let sandbox = tempfile::Builder::new()
            .prefix("lsp-cli-e2e-")
            .tempdir_in(test_temp_root)?;
        let test_temp_base = test_temp_base()?;
        fs::create_dir_all(&test_temp_base)?;
        // Keep this prefix and hierarchy short: daemon socket paths have a small OS limit.
        let runtime_sandbox = tempfile::Builder::new()
            .prefix("e-")
            .tempdir_in(test_temp_base)?;
        let home = sandbox.path().join("home");
        let config_home = sandbox.path().join("config");
        let workspace = sandbox.path().join("workspace");
        let bin_dir = sandbox.path().join("bin");
        let runtime_dir = runtime_sandbox.path().to_path_buf();

        for directory in [&home, &config_home, &workspace, &bin_dir] {
            fs::create_dir(directory)?;
        }

        Ok(Self {
            _sandbox: sandbox,
            _runtime_sandbox: runtime_sandbox,
            home,
            config_home,
            runtime_dir,
            workspace,
            bin_dir,
            data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"),
        })
    }

    pub(crate) fn run(&self, args: &[&str]) -> io::Result<Output> {
        self.command().args(args).output()
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lsp-cli"));
        command
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("XDG_RUNTIME_DIR", &self.runtime_dir)
            .env("LSP_DATA", &self.data_dir)
            .env("PATH", &self.bin_dir)
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("TZ", "UTC")
            .current_dir(&self.workspace);
        command
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;

    use super::*;

    #[test]
    fn command_isolated_from_ambient_process_state() {
        let context = E2eContext::new().expect("E2E context should initialize");
        let command = context.command();
        let actual = command
            .get_envs()
            .map(|(name, value)| (name.to_os_string(), value.map(OsString::from)))
            .collect::<BTreeMap<_, _>>();
        let expected = [
            ("HOME", context.home.as_os_str().to_os_string()),
            ("LANG", OsString::from("C")),
            ("LC_ALL", OsString::from("C")),
            ("LSP_DATA", context.data_dir.as_os_str().to_os_string()),
            ("PATH", context.bin_dir.as_os_str().to_os_string()),
            ("TZ", OsString::from("UTC")),
            (
                "XDG_CONFIG_HOME",
                context.config_home.as_os_str().to_os_string(),
            ),
            (
                "XDG_RUNTIME_DIR",
                context.runtime_dir.as_os_str().to_os_string(),
            ),
        ]
        .into_iter()
        .map(|(name, value)| (OsString::from(name), Some(value)))
        .collect::<BTreeMap<_, _>>();

        assert_eq!(actual, expected);
        assert_eq!(command.get_current_dir(), Some(context.workspace.as_path()));
        assert!(!context.runtime_dir.starts_with(context._sandbox.path()));
    }

    #[cfg(unix)]
    #[test]
    fn runtime_directory_has_room_for_daemon_socket_name() {
        use std::os::unix::net::UnixListener;

        let context = E2eContext::new().expect("E2E context should initialize");
        let daemon_root = context.runtime_dir.join("lsp-cli");
        fs::create_dir(&daemon_root).expect("daemon root should be created");
        let socket_path = daemon_root.join(format!("{}-{}.sock", "s".repeat(32), "f".repeat(24)));
        let _listener = UnixListener::bind(&socket_path).unwrap_or_else(|error| {
            panic!(
                "E2E runtime path {} cannot hold a daemon socket: {error}",
                socket_path.display()
            )
        });
    }
}
