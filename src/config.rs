use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, de};

#[derive(Debug)]
pub struct ConfigStore {
    pub filetypes: Vec<FiletypeConfig>,
    pub lsps: Vec<LspConfig>,
    pub cli: CliConfig,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CliConfig {
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
    #[serde(default, rename = "lsp")]
    lsp_preferences: BTreeMap<String, Vec<String>>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DetectCliConfigFile {
    #[serde(default)]
    quiet: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct DaemonCliConfigFile {
    #[serde(
        rename = "idle-timeout",
        default,
        deserialize_with = "deserialize_optional_timeout"
    )]
    idle_timeout: Option<Duration>,
}

pub fn default_config_root() -> Result<PathBuf, String> {
    let lsp_data = env::var_os("LSP_DATA").map(PathBuf::from);
    let home = env::var_os("HOME").map(PathBuf::from);
    let repo_data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");

    choose_config_root(lsp_data.as_deref(), home.as_deref(), &repo_data)
}

pub fn default_cli_config_roots() -> (PathBuf, Option<PathBuf>) {
    let global = env::var_os("LSP_DATA").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"),
        PathBuf::from,
    );
    let xdg_config_home = env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let home = env::var_os("HOME").map(PathBuf::from);
    let user = choose_cli_config_user_root(xdg_config_home.as_deref(), home.as_deref());
    (global, user)
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
) -> Result<PathBuf, String> {
    if let Some(path) = lsp_data {
        return Ok(path.to_path_buf());
    }

    if let Some(home) = home {
        let home_root = home.join(".local/share/lsp-cli");
        if has_config_dirs(&home_root) {
            return Ok(home_root);
        }
    }

    if has_config_dirs(repo_data) {
        return Ok(repo_data.to_path_buf());
    }

    Err(
        "could not resolve config root from LSP_DATA, ~/.local/share/lsp-cli, or repo data/"
            .to_string(),
    )
}

fn has_config_dirs(root: &Path) -> bool {
    root.join("filetypes").is_dir() && root.join("lsp").is_dir()
}

pub fn load_config_store(root: &Path) -> Result<ConfigStore, String> {
    let filetypes = load_filetypes(&root.join("filetypes"))?;
    let lsps = load_lsps(&root.join("lsp"))?;
    validate_lsp_filetypes(&filetypes, &lsps)?;

    Ok(ConfigStore {
        filetypes,
        lsps,
        cli: CliConfig::default(),
    })
}

pub fn load_cli_config(global_root: &Path, user_root: Option<&Path>) -> Result<CliConfig, String> {
    let mut config = CliConfig::default();
    config.merge(load_optional_cli_config_file(
        &global_root.join("lsp-cli.yaml"),
    )?);

    if let Some(user_root) = user_root {
        config.merge(load_optional_cli_config_file(
            &user_root.join("lsp-cli.yaml"),
        )?);
    }

    Ok(config)
}

fn load_optional_cli_config_file(path: &Path) -> Result<CliConfig, String> {
    if !path.exists() {
        return Ok(CliConfig::default());
    }

    let contents =
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let file: CliConfigFile =
        serde_yaml::from_str(&contents).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(CliConfig::from(file))
}

impl CliConfig {
    fn merge(&mut self, other: Self) {
        if other.download.is_some() {
            self.download = other.download;
        }
        if other.detach.is_some() {
            self.detach = other.detach;
        }
        if other.json.is_some() {
            self.json = other.json;
        }
        if other.debug.is_some() {
            self.debug = other.debug;
        }
        if other.timeout.is_some() {
            self.timeout = other.timeout;
        }
        if other.limit.is_some() {
            self.limit = other.limit;
        }
        if other.detect.quiet.is_some() {
            self.detect.quiet = other.detect.quiet;
        }
        if other.daemon.idle_timeout.is_some() {
            self.daemon.idle_timeout = other.daemon.idle_timeout;
        }
        self.lsp_preferences.extend(other.lsp_preferences);
    }
}

impl From<CliConfigFile> for CliConfig {
    fn from(file: CliConfigFile) -> Self {
        Self {
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
            lsp_preferences: file.lsp_preferences,
        }
    }
}

pub(crate) fn parse_timeout(value: &str) -> Result<Duration, String> {
    if let Some(milliseconds) = value.strip_suffix("ms") {
        let milliseconds = milliseconds.parse::<u64>().map_err(|_| {
            format!("invalid timeout {value:?}: expected integer milliseconds or seconds")
        })?;
        return Ok(Duration::from_millis(milliseconds));
    }

    let seconds = value.parse::<f64>().map_err(|_| {
        format!("invalid timeout {value:?}: expected integer milliseconds or seconds")
    })?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(format!(
            "invalid timeout {value:?}: expected non-negative milliseconds or seconds"
        ));
    }

    Ok(Duration::from_secs_f64(seconds))
}

fn deserialize_optional_timeout<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_timeout(&value).map_err(de::Error::custom))
        .transpose()
}

fn load_filetypes(dir: &Path) -> Result<Vec<FiletypeConfig>, String> {
    let paths = yaml_files_in(dir)?;

    paths
        .into_iter()
        .map(|path| {
            let contents = fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let file: FiletypeFile = serde_yaml::from_str(&contents)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("invalid filetype filename: {}", path.display()))?
                .to_string();
            let patterns = file
                .patterns
                .into_iter()
                .map(|pattern| {
                    Regex::new(&pattern).map_err(|error| {
                        format!("{}: invalid regex {pattern:?}: {error}", path.display())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

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

fn load_lsps(dir: &Path) -> Result<Vec<LspConfig>, String> {
    let paths = yaml_files_in(dir)?;

    paths
        .into_iter()
        .map(|path| {
            let contents = fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let file: LspFile = serde_yaml::from_str(&contents)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("invalid lsp filename: {}", path.display()))?
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

fn yaml_files_in(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !dir.exists() {
        return Err(format!("missing directory {}", dir.display()));
    }

    if !dir.is_dir() {
        return Err(format!("not a directory: {}", dir.display()));
    }

    let mut paths = fs::read_dir(dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", dir.display()))?;

    paths.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("yaml"));
    paths.sort();

    if paths.is_empty() {
        return Err(format!("no yaml files found in {}", dir.display()));
    }

    Ok(paths)
}

fn validate_lsp_filetypes(filetypes: &[FiletypeConfig], lsps: &[LspConfig]) -> Result<(), String> {
    let known_filetypes = filetypes
        .iter()
        .map(|filetype| filetype.id.clone())
        .collect::<BTreeSet<_>>();

    for lsp in lsps {
        for filetype in &lsp.filetypes {
            if !known_filetypes.contains(filetype) {
                return Err(format!(
                    "lsp {} references unknown filetype {}",
                    lsp.name, filetype
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
