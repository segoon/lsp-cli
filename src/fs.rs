use std::fmt;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

pub(crate) fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| Error::unexpected(format!("failed to create directory {}: {error}", path.display())))
}

pub(crate) fn metadata(path: &Path) -> Result<fs::Metadata> {
    fs::metadata(path)
        .map_err(|error| Error::unexpected(format!("failed to get file metadata {}: {error}", path.display())))
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path)
        .map_err(|error| Error::unexpected(format!("failed to read {}: {error}", path.display())))
}

pub(crate) fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map_err(|error| Error::unexpected(format!("failed to read {}: {error}", path.display())))
}

pub(crate) fn read_dir(path: &Path) -> Result<fs::ReadDir> {
    fs::read_dir(path)
        .map_err(|error| Error::unexpected(format!("failed to read directory entries {}: {error}", path.display())))
}

pub(crate) fn set_permissions(path: &Path, permissions: fs::Permissions) -> Result<()> {
    fs::set_permissions(path, permissions)
        .map_err(|error| Error::unexpected(format!("failed to set permissions on {}: {error}", path.display())))
}

pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes)
        .map_err(|error| Error::unexpected(format!("failed to write {}: {error}", path.display())))
}

pub(crate) fn format_path_error(path: &Path, error: impl fmt::Display) -> String {
    format!("{}: {error}", path.display())
}
