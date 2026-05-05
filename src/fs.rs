use std::fmt;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result, error_fn};

pub(crate) fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(error_fn!(
        Error::unexpected,
        "failed to create directory {}",
        path.display()
    ))
}

pub(crate) fn metadata(path: &Path) -> Result<fs::Metadata> {
    fs::metadata(path).map_err(error_fn!(
        Error::unexpected,
        "failed to get file metadata {}",
        path.display()
    ))
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(error_fn!(
        Error::unexpected,
        "failed to read {}",
        path.display()
    ))
}

pub(crate) fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(error_fn!(
        Error::unexpected,
        "failed to read {}",
        path.display()
    ))
}

pub(crate) fn read_dir(path: &Path) -> Result<fs::ReadDir> {
    fs::read_dir(path).map_err(error_fn!(
        Error::unexpected,
        "failed to read directory entries {}",
        path.display()
    ))
}

pub(crate) fn set_permissions(path: &Path, permissions: fs::Permissions) -> Result<()> {
    fs::set_permissions(path, permissions).map_err(error_fn!(
        Error::unexpected,
        "failed to set permissions on {}",
        path.display()
    ))
}

pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).map_err(error_fn!(
        Error::unexpected,
        "failed to write {}",
        path.display()
    ))
}

pub(crate) fn format_path_error(path: &Path, error: impl fmt::Display) -> String {
    format!("{}: {error}", path.display())
}
