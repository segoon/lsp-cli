use std::path::Path;

use tempfile::TempDir;

pub(crate) const LOCAL_SHARE_LSP_CLI: &str = ".local/share/lsp-cli";

pub(crate) struct TestDir {
    directory: TempDir,
}

impl TestDir {
    pub(crate) fn new(prefix: &str) -> Self {
        Self {
            directory: tempfile::Builder::new()
                .prefix(prefix)
                .tempdir()
                .expect("test directory should initialize"),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        self.directory.path()
    }
}
