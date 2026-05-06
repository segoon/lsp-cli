use std::fs;
use std::path::{Path, PathBuf};

use lsp_types::Uri;
use url::Url;

use crate::error::{Error, Result, error_fn};

pub fn path_to_file_uri(path: &Path) -> Result<String> {
    let absolute = fs::canonicalize(path).map_err(error_fn!(
        Error::unexpected,
        "failed to resolve {}",
        path.display()
    ))?;

    let url = if absolute.is_dir() {
        Url::from_directory_path(&absolute)
    } else {
        Url::from_file_path(&absolute)
    }
    .map_err(|()| {
        Error::unexpected(format!(
            "failed to build file URI for {}",
            absolute.display()
        ))
    })?;

    Ok(url.to_string())
}

pub fn file_uri_to_path(uri: &str) -> Result<PathBuf> {
    let url = Url::parse(uri).map_err(error_fn!(Error::lsp, "invalid location URI {:?}", uri))?;

    url.to_file_path()
        .map_err(|()| Error::lsp(format!("workspace/symbol returned non-file URI {uri:?}")))
}

pub fn parse_lsp_uri(uri: &str, context: &str) -> Result<Uri> {
    uri.parse()
        .map_err(error_fn!(Error::lsp, "invalid {} URI {:?}", context, uri))
}

pub fn workspace_name(path: &Path) -> String {
    if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
        name.to_string()
    } else {
        path.display().to_string()
    }
}
