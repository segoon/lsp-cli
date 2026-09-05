use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::TempDir;

#[path = "../../src/test_support/temp_root.rs"]
mod temp_root;

use self::temp_root::test_temp_root;

pub(crate) struct E2eContext {
    _sandbox: TempDir,
    home: PathBuf,
    config_home: PathBuf,
    runtime_dir: PathBuf,
    data_dir: PathBuf,
}

impl E2eContext {
    pub(crate) fn new() -> io::Result<Self> {
        let test_temp_root = test_temp_root()?;
        fs::create_dir_all(&test_temp_root)?;
        let sandbox = tempfile::Builder::new()
            .prefix("lsp-cli-e2e-")
            .tempdir_in(test_temp_root)?;
        let home = sandbox.path().join("home");
        let config_home = sandbox.path().join("config");
        let runtime_dir = sandbox.path().join("runtime");

        for directory in [&home, &config_home, &runtime_dir] {
            fs::create_dir(directory)?;
        }

        Ok(Self {
            _sandbox: sandbox,
            home,
            config_home,
            runtime_dir,
            data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"),
        })
    }

    pub(crate) fn run(&self, args: &[&str]) -> io::Result<Output> {
        self.command().args(args).output()
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lsp-cli"));
        command
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.config_home)
            .env("XDG_RUNTIME_DIR", &self.runtime_dir)
            .env("LSP_DATA", &self.data_dir);
        command
    }
}
