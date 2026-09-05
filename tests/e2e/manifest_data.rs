use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::case_files::{file_stem, read_yaml, yaml_paths};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PairKey {
    pub(crate) language: String,
    pub(crate) server: String,
}

#[derive(Deserialize)]
pub(crate) struct FiletypeConfig {
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    patterns: Vec<String>,
}

impl FiletypeConfig {
    pub(crate) fn is_detectable(&self) -> bool {
        !self.extensions.is_empty() || !self.patterns.is_empty()
    }
}

#[derive(Deserialize)]
pub(crate) struct LspConfig {
    #[serde(default)]
    pub(crate) filetypes: Vec<String>,
    pub(crate) name: String,
}

pub(crate) fn detectable_languages(data: &Path) -> Result<BTreeSet<String>, String> {
    let mut languages = BTreeSet::new();
    for path in yaml_paths(&data.join("filetypes"))? {
        let config: FiletypeConfig = read_yaml(&path)?;
        if config.is_detectable() {
            languages.insert(file_stem(&path)?);
        }
    }
    Ok(languages)
}

pub(crate) fn compatible_pairs(
    data: &Path,
    detectable: &BTreeSet<String>,
) -> Result<BTreeSet<PairKey>, String> {
    let mut pairs = BTreeSet::new();
    for path in yaml_paths(&data.join("lsp"))? {
        let config: LspConfig = read_yaml(&path)?;
        let server = file_stem(&path)?;
        for language in config.filetypes {
            if detectable.contains(&language) {
                pairs.insert(PairKey {
                    language,
                    server: server.clone(),
                });
            }
        }
    }
    Ok(pairs)
}
