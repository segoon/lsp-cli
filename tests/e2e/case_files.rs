use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

pub(crate) fn yaml_paths(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("failed to read {}: {error}", directory.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().and_then(|extension| extension.to_str()) == Some("yaml"));
    paths.sort();
    Ok(paths)
}

pub(crate) fn read_yaml<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_yaml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

pub(crate) fn file_stem(path: &Path) -> Result<String, String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("{} has no UTF-8 file stem", path.display()))
}
