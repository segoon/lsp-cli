use std::fmt;
use std::fs;
use std::path::Path;

pub(crate) fn create_dir_all(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("failed to create directory {}: {error}", path.display()))
}

pub(crate) fn metadata(path: &Path) -> Result<fs::Metadata, String> {
    fs::metadata(path)
        .map_err(|error| format!("failed to get file metadata {}: {error}", path.display()))
}

pub(crate) fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

pub(crate) fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))
}

pub(crate) fn read_dir(path: &Path) -> Result<fs::ReadDir, String> {
    fs::read_dir(path)
        .map_err(|error| format!("failed to read directory entries {}: {error}", path.display()))
}

pub(crate) fn set_permissions(path: &Path, permissions: fs::Permissions) -> Result<(), String> {
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to set permissions on {}: {error}", path.display()))
}

pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(crate) fn format_path_error(path: &Path, error: impl fmt::Display) -> String {
    format!("{}: {error}", path.display())
}
