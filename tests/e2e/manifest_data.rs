use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Deserialize)]
struct CliConfig {
    #[serde(default)]
    lsp: BTreeMap<String, Vec<String>>,
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

pub(crate) fn preferred_pairs(
    data: &Path,
    languages: &BTreeSet<String>,
) -> Result<BTreeSet<PairKey>, String> {
    let config: CliConfig = read_yaml(&data.join("lsp-cli.yaml"))?;
    let mut lsps = Vec::new();
    for path in yaml_paths(&data.join("lsp"))? {
        lsps.push((file_stem(&path)?, read_yaml::<LspConfig>(&path)?));
    }

    languages
        .iter()
        .map(|language| {
            let preferred_name = config
                .lsp
                .get(language)
                .and_then(|servers| servers.first())
                .ok_or_else(|| {
                    format!("E2E source language {language:?} has no preferred server in data")
                })?;
            let matching = lsps
                .iter()
                .filter(|(_id, lsp)| {
                    lsp.name == *preferred_name && lsp.filetypes.contains(language)
                })
                .collect::<Vec<_>>();
            match matching.as_slice() {
                [(server, _lsp)] => Ok(PairKey {
                    language: language.clone(),
                    server: server.clone(),
                }),
                [] => Err(format!(
                    "preferred server {preferred_name:?} for E2E source language {language:?} has no compatible LSP config"
                )),
                _ => Err(format!(
                    "preferred server {preferred_name:?} for E2E source language {language:?} is ambiguous"
                )),
            }
        })
        .collect()
}
