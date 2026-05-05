use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::env_vars;
use crate::fs as path_fs;
use regex::Regex;
use serde::{Deserialize, de};

#[derive(Debug)]
pub struct ConfigStore {
    pub filetypes: Vec<FiletypeConfig>,
    pub lsps: Vec<LspConfig>,
    pub cli: CliConfig,
}

pub struct CliConfigRoots {
    pub global: PathBuf,
    pub user: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CliConfig {
    pub download_version: Option<String>,
    pub download: Option<bool>,
    pub detach: Option<bool>,
    pub json: Option<bool>,
    pub debug: Option<bool>,
    pub timeout: Option<Duration>,
    pub limit: Option<usize>,
    pub detect: DetectCliConfig,
    pub daemon: DaemonCliConfig,
    pub lsp_preferences: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DetectCliConfig {
    pub quiet: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DaemonCliConfig {
    pub idle_timeout: Option<Duration>,
}

#[derive(Debug)]
pub struct FiletypeConfig {
    pub id: String,
    pub extensions: Vec<String>,
    pub patterns: Vec<Regex>,
}

#[derive(Debug)]
pub struct LspConfig {
    pub id: String,
    pub filetypes: Vec<String>,
    pub root_markers: Vec<String>,
    pub name: String,
    pub cmdline: String,
    pub wait_for_index: bool,
}

#[derive(Deserialize)]
struct FiletypeFile {
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    patterns: Vec<String>,
}

#[derive(Deserialize)]
struct LspFile {
    #[serde(default)]
    filetypes: Vec<String>,
    #[serde(default)]
    root_markers: Vec<String>,
    name: String,
    cmdline: String,
    #[serde(rename = "wait-for-index", default)]
    wait_for_index: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CliConfigFile {
    #[serde(default, rename = "download-version")]
    download_version: Option<String>,
    #[serde(default)]
    download: Option<bool>,
    #[serde(default)]
    detach: Option<bool>,
    #[serde(default)]
    json: Option<bool>,
    #[serde(default)]
    debug: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_timeout")]
    timeout: Option<Duration>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    detect: DetectCliConfigFile,
    #[serde(default)]
    daemon: DaemonCliConfigFile,
    // QD: why not simply name it 'lsp'?
    // A: The YAML key is now `lsp`, but the runtime config still uses
    // A: `lsp_preferences` to avoid confusion with one selected LSP.
    #[serde(default, rename = "lsp")]
    lsp: BTreeMap<String, Vec<String>>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DetectCliConfigFile {
    #[serde(default)]
    quiet: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct DaemonCliConfigFile {
    #[serde(default, deserialize_with = "deserialize_optional_timeout")]
    idle_timeout: Option<Duration>,
}

pub fn default_config_root() -> Result<PathBuf> {
    let lsp_data = env_vars::lsp_data();
    let home = env_vars::home();
    let repo_data = repo_data_dir();

    choose_config_root(lsp_data.as_deref(), home.as_deref(), &repo_data)
}

fn choose_cli_config_user_root(
    xdg_config_home: Option<&Path>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    xdg_config_home
        .map(|path| path.join("lsp-cli"))
        .or_else(|| home.map(|path| path.join(".config/lsp-cli")))
}

fn choose_config_root(
    lsp_data: Option<&Path>,
    home: Option<&Path>,
    repo_data: &Path,
) -> Result<PathBuf> {
    // 1. Respect an explicit `LSP_DATA` override.
    if let Some(path) = lsp_data {
        return Ok(path.to_path_buf());
    }

    if let Some(home) = home {
        // 2. Prefer previously downloaded per-user data when it looks complete.
        let downloaded_root = home_data_dir(home);
        if has_config_dirs(&downloaded_root) {
            return Ok(downloaded_root);
        }
    }

    // 3. Fall back to the repository `data/` tree for development and bootstrapping.
    if has_config_dirs(repo_data) {
        return Ok(repo_data.to_path_buf());
    }

    Err(Error::unexpected(
        "could not resolve config root from LSP_DATA, ~/.local/share/lsp-cli/data, or repo data/",
    ))
}

fn has_config_dirs(root: &Path) -> bool {
    root.join("filetypes").is_dir() && root.join("lsp").is_dir()
}

pub fn load_config_store(root: &Path) -> Result<ConfigStore> {
    let root = ConfigRoot::new(root);
    let filetypes = load_filetypes(&root.filetypes_dir())?;
    let lsps = load_lsps(&root.lsp_dir())?;
    validate_lsp_filetypes(&filetypes, &lsps)?;

    Ok(ConfigStore {
        filetypes,
        lsps,
        cli: CliConfig::default(),
    })
}

pub fn load_cli_config(global_root: &Path, user_root: Option<&Path>) -> Result<CliConfig> {
    let mut config = CliConfig::default();
    config.merge(load_optional_cli_config_file(
        &ConfigRoot::new(global_root).cli_config_path(),
    )?);

    if let Some(user_root) = user_root {
        config.merge(load_optional_cli_config_file(
            &ConfigRoot::new(user_root).cli_config_path(),
        )?);
    }

    Ok(config)
}

fn load_optional_cli_config_file(path: &Path) -> Result<CliConfig> {
    if !path.exists() {
        return Ok(CliConfig::default());
    }

    let contents = path_fs::read_to_string(path)?;
    let file: CliConfigFile = serde_yaml::from_str(&contents)
        .map_err(|error| Error::config_format(path_fs::format_path_error(path, error)))?;
    Ok(CliConfig::from(file))
}

impl CliConfig {
    fn merge(&mut self, other: Self) {
        override_if_some(&mut self.download, other.download);
        override_if_some(&mut self.detach, other.detach);
        override_if_some(&mut self.json, other.json);
        override_if_some(&mut self.debug, other.debug);
        override_if_some(&mut self.timeout, other.timeout);
        override_if_some(&mut self.limit, other.limit);
        override_if_some(&mut self.detect.quiet, other.detect.quiet);
        override_if_some(&mut self.daemon.idle_timeout, other.daemon.idle_timeout);
        override_if_some(&mut self.download_version, other.download_version);
        self.lsp_preferences.extend(other.lsp_preferences);
    }
}

impl From<CliConfigFile> for CliConfig {
    fn from(file: CliConfigFile) -> Self {
        Self {
            download_version: file.download_version,
            download: file.download,
            detach: file.detach,
            json: file.json,
            debug: file.debug,
            timeout: file.timeout,
            limit: file.limit,
            detect: DetectCliConfig {
                quiet: file.detect.quiet,
            },
            daemon: DaemonCliConfig {
                idle_timeout: file.daemon.idle_timeout,
            },
            lsp_preferences: file.lsp,
        }
    }
}

fn parse_timeout_value(value: &str) -> Result<Duration> {
    if let Some(milliseconds) = value.strip_suffix("ms") {
        let milliseconds = milliseconds.parse::<u64>().map_err(|_| {
            Error::invalid_input(format!(
                "invalid timeout {value:?}: expected integer milliseconds or seconds"
            ))
        })?;
        return Ok(Duration::from_millis(milliseconds));
    }

    let seconds = value.parse::<f64>().map_err(|_| {
        Error::invalid_input(format!(
            "invalid timeout {value:?}: expected integer milliseconds or seconds"
        ))
    })?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(Error::invalid_input(format!(
            "invalid timeout {value:?}: expected non-negative milliseconds or seconds"
        )));
    }

    Ok(Duration::from_secs_f64(seconds))
}

pub(crate) fn parse_timeout(value: &str) -> std::result::Result<Duration, String> {
    // Clap and serde parser hooks expect string/custom errors, so keep this as a thin adapter
    // over the typed parser instead of duplicating parsing logic here.
    parse_timeout_value(value).map_err(|error| error.to_string())
}

fn deserialize_optional_timeout<'de, D>(deserializer: D) -> std::result::Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_timeout_value(&value).map_err(|error| de::Error::custom(error.to_string())))
        .transpose()
}

fn load_filetypes(dir: &Path) -> Result<Vec<FiletypeConfig>> {
    let paths = find_yaml_files_in(dir)?;

    paths
        .into_iter()
        .map(|path| {
            let contents = path_fs::read_to_string(&path)?;
            let file: FiletypeFile = serde_yaml::from_str(&contents)
                .map_err(|error| Error::config_format(path_fs::format_path_error(&path, error)))?;
            let id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| Error::config_format(format!("invalid filetype filename: {}", path.display())))?
                .to_string();
            let patterns = file
                .patterns
                .into_iter()
                .map(|pattern| {
                    Regex::new(&pattern).map_err(|error| {
                        Error::config_format(format!("{}: invalid regex {pattern:?}: {error}", path.display()))
                    })
                })
                .collect::<std::result::Result<Vec<_>, _>>()?;

            Ok(FiletypeConfig {
                id,
                extensions: file
                    .extensions
                    .into_iter()
                    .map(|extension| extension.to_ascii_lowercase())
                    .collect(),
                patterns,
            })
        })
        .collect()
}

fn load_lsps(dir: &Path) -> Result<Vec<LspConfig>> {
    let paths = find_yaml_files_in(dir)?;

    paths
        .into_iter()
        .map(|path| {
            let contents = path_fs::read_to_string(&path)?;
            let file: LspFile = serde_yaml::from_str(&contents)
                .map_err(|error| Error::config_format(path_fs::format_path_error(&path, error)))?;
            let id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| Error::config_format(format!("invalid lsp filename: {}", path.display())))?
                .to_string();

            Ok(LspConfig {
                id,
                filetypes: file.filetypes,
                root_markers: file.root_markers,
                name: file.name,
                cmdline: file.cmdline,
                wait_for_index: file.wait_for_index,
            })
        })
        .collect()
}

fn find_yaml_files_in(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Err(Error::unexpected(format!("missing directory {}", dir.display())));
    }

    if !dir.is_dir() {
        return Err(Error::unexpected(format!("not a directory: {}", dir.display())));
    }

    let mut paths = fs::read_dir(dir)
        .map_err(|error| Error::unexpected(format!("{}: {error}", dir.display())))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| Error::unexpected(format!("{}: {error}", dir.display())))?;

    paths.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("yaml"));
    paths.sort();

    if paths.is_empty() {
        return Err(Error::unexpected(format!("no yaml files found in {}", dir.display())));
    }

    Ok(paths)
}

fn validate_lsp_filetypes(filetypes: &[FiletypeConfig], lsps: &[LspConfig]) -> Result<()> {
    let known_filetypes = filetypes
        .iter()
        .map(|filetype| filetype.id.clone())
        .collect::<BTreeSet<_>>();

    for lsp in lsps {
        for filetype in &lsp.filetypes {
            if !known_filetypes.contains(filetype) {
                return Err(Error::config_format(format!(
                    "lsp {} references unknown filetype {}",
                    lsp.name, filetype
                )));
            }
        }
    }

    Ok(())
}

fn repo_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn home_data_dir(home: &Path) -> PathBuf {
    home.join(".local/share/lsp-cli/data")
}

fn override_if_some<T>(target: &mut Option<T>, replacement: Option<T>) {
    if replacement.is_some() {
        *target = replacement;
    }
}

struct ConfigRoot<'a> {
    root: &'a Path,
}

impl Default for CliConfigRoots {
    fn default() -> Self {
        let home = env_vars::home();
        let repo_data = repo_data_dir();
        let global = env_vars::lsp_data().unwrap_or_else(|| {
            home.as_deref().map_or_else(
                || repo_data.clone(),
                |home| {
                    let user_data = home_data_dir(home);
                    if has_config_dirs(&user_data) {
                        user_data
                    } else {
                        repo_data.clone()
                    }
                },
            )
        });

        Self {
            global,
            user: choose_cli_config_user_root(env_vars::xdg_config_home().as_deref(), home.as_deref()),
        }
    }
}

impl<'a> ConfigRoot<'a> {
    fn new(root: &'a Path) -> Self {
        Self { root }
    }

    fn filetypes_dir(&self) -> PathBuf {
        self.root.join("filetypes")
    }

    fn lsp_dir(&self) -> PathBuf {
        self.root.join("lsp")
    }

    fn cli_config_path(&self) -> PathBuf {
        self.root.join("lsp-cli.yaml")
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
