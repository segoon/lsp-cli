use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use crate::harness::E2eContext;
use crate::process;

const SERVER_NAME: &str = "e2e-fake-lsp";
const BINARY_NAME: &str = "lsp-cli-e2e-fake-lsp";
const BUILD_DEADLINE: Duration = Duration::from_secs(120);

pub(crate) struct LocalFixture {
    context: E2eContext,
}

impl LocalFixture {
    pub(crate) fn new() -> Result<Self, String> {
        let root = repository_root();
        let fixtures = root.join("tests/e2e/fixtures");
        let context = E2eContext::new()
            .map_err(|error| format!("failed to create fixture context: {error}"))?
            .with_data_dir(fixtures.join("data"));
        context.copy_project(&fixtures.join("project"))?;
        context.stage_program(SERVER_NAME, fake_server_binary()?)?;
        Ok(Self { context })
    }

    pub(crate) fn context(&self) -> &E2eContext {
        &self.context
    }

    pub(crate) fn server_name(&self) -> &'static str {
        SERVER_NAME
    }
}

fn fake_server_binary() -> Result<&'static Path, String> {
    static BINARY: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    match BINARY.get_or_init(build_fake_server) {
        Ok(path) => Ok(path),
        Err(error) => Err(error.clone()),
    }
}

fn build_fake_server() -> Result<PathBuf, String> {
    let root = repository_root();
    let manifest = root.join("tests/e2e/fixtures/fake-lsp/Cargo.toml");
    let target = root.join("target/e2e-fixtures");
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    let mut command = Command::new(cargo);
    command.args([
        "build",
        "--quiet",
        "--locked",
        "--manifest-path",
        manifest
            .to_str()
            .ok_or_else(|| format!("fixture manifest path is not UTF-8: {}", manifest.display()))?,
        "--target-dir",
        target
            .to_str()
            .ok_or_else(|| format!("fixture target path is not UTF-8: {}", target.display()))?,
    ]);
    let output = process::run(&mut command, BUILD_DEADLINE)
        .map_err(|failure| failure.diagnostic("fake LSP build has no daemon runtime"))?;
    if !output.status().success() {
        return Err(output.diagnostic(
            "failed to build the fake LSP fixture",
            "fake LSP build has no daemon runtime",
        ));
    }
    let binary = target.join("debug").join(BINARY_NAME);
    if binary.is_file() {
        Ok(binary)
    } else {
        Err(format!(
            "fake LSP build did not create {}",
            binary.display()
        ))
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
