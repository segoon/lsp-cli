use std::fs;
use std::path::{Path, PathBuf};

use url::Url;

pub fn path_to_file_uri(path: &Path) -> Result<String, String> {
    let absolute = fs::canonicalize(path)
        .map_err(|error| format!("failed to resolve {}: {error}", path.display()))?;

    let url = if absolute.is_dir() {
        Url::from_directory_path(&absolute)
    } else {
        Url::from_file_path(&absolute)
    }
    .map_err(|()| format!("failed to build file URI for {}", absolute.display()))?;

    Ok(url.to_string())
}

pub fn file_uri_to_path(uri: &str) -> Result<PathBuf, String> {
    let url = Url::parse(uri).map_err(|error| format!("invalid location URI {uri:?}: {error}"))?;

    url.to_file_path()
        .map_err(|()| format!("workspace/symbol returned non-file URI {uri:?}"))
}

pub fn workspace_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map_or_else(|| path.display().to_string(), ToString::to_string)
}
