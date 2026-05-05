use std::fmt;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

fn error_fn(prefix: &str, path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    move |error| Error::unexpected(format!("{} {}: {error}", prefix, path.display()))
}

pub(crate) fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(error_fn("failed to create directory", path))
}

pub(crate) fn metadata(path: &Path) -> Result<fs::Metadata> {
    fs::metadata(path).map_err(error_fn("failed to get file metadata", path))
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(error_fn("failed to read", path))
}

pub(crate) fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(error_fn("failed to read", path))
}

pub(crate) fn read_dir(path: &Path) -> Result<fs::ReadDir> {
    fs::read_dir(path).map_err(error_fn("failed to read directory entries", path))
}

pub(crate) fn set_permissions(path: &Path, permissions: fs::Permissions) -> Result<()> {
    fs::set_permissions(path, permissions).map_err(error_fn("failed to set permissions on", path))
}

pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).map_err(error_fn("failed to write", path))
}

pub(crate) fn format_path_error(path: &Path, error: impl fmt::Display) -> String {
    format!("{}: {error}", path.display())
}
