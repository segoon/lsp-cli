use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn test_temp_root() -> io::Result<PathBuf> {
    Ok(test_temp_base()?.join("lsp-cli/test-tmp"))
}

pub(crate) fn test_temp_base() -> io::Result<PathBuf> {
    option_env!("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| acceptable_test_temp_base(path))
        .or_else(|| option_env!("XDG_CACHE_HOME").map(PathBuf::from))
        .filter(|path| acceptable_test_temp_base(path))
        .or_else(|| option_env!("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .filter(|path| acceptable_test_temp_base(path))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "an absolute XDG_RUNTIME_DIR, XDG_CACHE_HOME, or HOME outside /tmp is required",
            )
        })
}

fn acceptable_test_temp_base(path: &Path) -> bool {
    path.is_absolute() && !path.starts_with("/tmp")
}
