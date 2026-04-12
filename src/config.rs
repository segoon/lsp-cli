use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Deserialize;

#[derive(Debug)]
pub struct ConfigStore {
    pub filetypes: Vec<FiletypeConfig>,
    pub lsps: Vec<LspConfig>,
}

#[derive(Debug)]
pub struct FiletypeConfig {
    pub id: String,
    pub extensions: Vec<String>,
    pub patterns: Vec<Regex>,
}

#[derive(Debug)]
pub struct LspConfig {
    pub filetypes: Vec<String>,
    pub filepatterns: Vec<String>,
    pub name: String,
    pub cmdline: String,
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
    filepatterns: Vec<String>,
    name: String,
    cmdline: String,
}

pub fn default_config_root() -> Result<PathBuf, String> {
    let lsp_data = env::var_os("LSP_DATA").map(PathBuf::from);
    let home = env::var_os("HOME").map(PathBuf::from);
    let repo_data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");

    choose_config_root(lsp_data.as_deref(), home.as_deref(), &repo_data)
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

    Ok(ConfigStore { filetypes, lsps })
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

            Ok(LspConfig {
                filetypes: file.filetypes,
                filepatterns: file.filepatterns,
                name: file.name,
                cmdline: file.cmdline,
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
mod tests {
    use super::{choose_config_root, default_config_root, load_config_store};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "lsp-cli-config-test-{}-{}",
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_file(&self, relative: &str, contents: &str) {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dirs should be created");
            }

            fs::write(path, contents).expect("file should be written");
        }

        fn writes_config_dirs(&self) {
            self.write_file(
                "filetypes/placeholder.yaml",
                "extensions: []\npatterns: []\n",
            );
            self.write_file(
                "lsp/placeholder.yaml",
                "filetypes: []\nfilepatterns: []\nname: placeholder\ncmdline: placeholder\n",
            );
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn resolves_config_root_from_lsp_data_env() {
        let lsp_data = TestDir::new();
        let home = TestDir::new();
        let repo = TestDir::new();
        lsp_data.write_file("filetypes/a.yaml", "extensions: []\npatterns: []\n");
        lsp_data.write_file(
            "lsp/a.yaml",
            "filetypes: []\nfilepatterns: []\nname: a\ncmdline: a\n",
        );
        home.write_file(
            ".local/share/lsp-cli/filetypes/b.yaml",
            "extensions: []\npatterns: []\n",
        );
        home.write_file(
            ".local/share/lsp-cli/lsp/b.yaml",
            "filetypes: []\nfilepatterns: []\nname: b\ncmdline: b\n",
        );
        repo.writes_config_dirs();

        assert_eq!(
            choose_config_root(Some(lsp_data.path()), Some(home.path()), repo.path())
                .expect("root should resolve"),
            lsp_data.path()
        );
    }

    #[test]
    fn falls_back_to_home_local_share() {
        let home = TestDir::new();
        let repo = TestDir::new();
        home.write_file(
            ".local/share/lsp-cli/filetypes/c.yaml",
            "extensions: []\npatterns: []\n",
        );
        home.write_file(
            ".local/share/lsp-cli/lsp/clangd.yaml",
            "filetypes: []\nfilepatterns: []\nname: clangd\ncmdline: clangd\n",
        );
        repo.writes_config_dirs();

        assert_eq!(
            choose_config_root(None, Some(home.path()), repo.path()).expect("root should resolve"),
            home.path().join(".local/share/lsp-cli")
        );
    }

    #[test]
    fn falls_back_to_repo_data_when_home_default_missing() {
        let home = TestDir::new();
        let repo = TestDir::new();
        repo.writes_config_dirs();

        assert_eq!(
            choose_config_root(None, Some(home.path()), repo.path()).expect("root should resolve"),
            repo.path()
        );
    }

    #[test]
    fn errors_when_no_root_can_be_resolved() {
        let home = TestDir::new();
        let repo = TestDir::new();

        let error = choose_config_root(None, Some(home.path()), repo.path())
            .expect_err("root resolution should fail");

        assert!(error.contains("could not resolve config root"));
    }

    #[test]
    fn default_config_root_resolves_in_real_environment() {
        let root = default_config_root().expect("root should resolve");

        assert!(root.ends_with(".local/share/lsp-cli") || root.ends_with("data"));
    }

    #[test]
    fn loads_valid_config_store() {
        let dir = TestDir::new();
        dir.write_file(
            "filetypes/c.yaml",
            "extensions:\n  - c\n  - h\npatterns:\n  - '^special$'\n",
        );
        dir.write_file("filetypes/cpp.yaml", "extensions:\n  - cpp\npatterns: []\n");
        dir.write_file(
            "lsp/clangd.yaml",
            concat!(
                "filetypes:\n",
                "  - c\n",
                "  - cpp\n",
                "filepatterns:\n",
                "  - compile_commands.json\n",
                "name: clangd\n",
                "cmdline: clangd --background-index $WORKSPACE\n"
            ),
        );

        let config = load_config_store(dir.path()).expect("config should load");

        assert_eq!(config.filetypes.len(), 2);
        assert_eq!(config.lsps.len(), 1);
        assert!(config.filetypes.iter().any(|filetype| filetype.id == "c"));
        assert_eq!(config.lsps[0].name, "clangd");
    }

    #[test]
    fn fails_when_config_root_is_missing() {
        let missing = std::env::temp_dir().join(format!(
            "lsp-cli-config-missing-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        ));

        let error = load_config_store(&missing).expect_err("config load should fail");

        assert!(error.contains("missing directory"));
    }

    #[test]
    fn fails_on_invalid_yaml() {
        let dir = TestDir::new();
        dir.write_file("filetypes/c.yaml", "extensions: [c\n");
        dir.write_file(
            "lsp/clangd.yaml",
            "filetypes: [c]\nfilepatterns: []\nname: clangd\ncmdline: clangd\n",
        );

        let error = load_config_store(dir.path()).expect_err("config load should fail");

        assert!(error.contains("filetypes/c.yaml"));
    }

    #[test]
    fn fails_on_unknown_lsp_filetype() {
        let dir = TestDir::new();
        dir.write_file("filetypes/c.yaml", "extensions: [c]\npatterns: []\n");
        dir.write_file(
            "lsp/clangd.yaml",
            "filetypes: [cpp]\nfilepatterns: []\nname: clangd\ncmdline: clangd\n",
        );

        let error = load_config_store(dir.path()).expect_err("config load should fail");

        assert!(error.contains("unknown filetype cpp"));
    }

    #[test]
    fn fails_on_invalid_regex() {
        let dir = TestDir::new();
        dir.write_file("filetypes/c.yaml", "extensions: [c]\npatterns: ['(']\n");
        dir.write_file(
            "lsp/clangd.yaml",
            "filetypes: [c]\nfilepatterns: []\nname: clangd\ncmdline: clangd\n",
        );

        let error = load_config_store(dir.path()).expect_err("config load should fail");

        assert!(error.contains("invalid regex"));
    }
}
