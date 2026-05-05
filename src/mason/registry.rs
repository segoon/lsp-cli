use crate::runtime_state::RuntimeState;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

mod cache;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MasonRegistry {
    lspconfigs: BTreeMap<String, MasonPackage>,
    package_names: BTreeMap<String, MasonPackage>,
    binaries: BTreeMap<String, String>,
}

impl MasonRegistry {
    pub fn load(state: &RuntimeState) -> Result<Self> {
        state.ensure_dirs()?;

        let registry_json_path = state.registry_json_path();
        match cache::ensure_registry_cache(state) {
            Ok(()) => Self::from_registry_json_path(&registry_json_path),
            Err(error) if registry_json_path.is_file() => {
                eprintln!("warning: failed to refresh Mason registry, using cached data: {error}");
                Self::from_registry_json_path(&registry_json_path)
            }
            Err(error) => Err(error),
        }
    }

    #[must_use]
    pub fn load_cached(state: &RuntimeState) -> Option<Self> {
        let path = state.registry_json_path();
        path.is_file()
            .then_some(())
            .and_then(|()| Self::from_registry_json_path(&path).ok())
    }

    pub fn package_for_lspconfig(&self, lspconfig: &str) -> Option<&MasonPackage> {
        self.lspconfigs.get(lspconfig)
    }

    pub fn package_for_detected(
        &self,
        config_id: &str,
        server: &str,
        program: &str,
    ) -> Option<&MasonPackage> {
        self.package_for_lspconfig(config_id)
            .or_else(|| {
                mapping_override(config_id).and_then(|target| self.package_for_override(target))
            })
            .or_else(|| self.package_for_name(config_id))
            .or_else(|| self.package_for_name(server))
            .or_else(|| self.package_for_bin(program))
    }

    fn package_for_name(&self, package_name: &str) -> Option<&MasonPackage> {
        self.package_names.get(package_name)
    }

    fn package_for_bin(&self, program: &str) -> Option<&MasonPackage> {
        self.binaries
            .get(program)
            .and_then(|package_name| self.package_for_name(package_name))
    }

    fn package_for_override(&self, target: MappingOverride) -> Option<&MasonPackage> {
        match target {
            MappingOverride::Lspconfig(lspconfig) => self.package_for_lspconfig(lspconfig),
            MappingOverride::Package(package_name) => self.package_for_name(package_name),
        }
    }

    fn from_registry_json_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .map_err(|error| Error::unexpected(format!("failed to read {}: {error}", path.display())))?;
        let package_values = serde_json::from_str::<Vec<serde_json::Value>>(&contents)
            .map_err(|error| Error::unexpected(format!("failed to parse {}: {error}", path.display())))?;
        let mut packages = Vec::new();
        for value in package_values.into_iter().filter(is_lsp_package_value) {
            if let Ok(mut package) = serde_json::from_value::<MasonPackage>(value) {
                let _ = package.apply_source_version_overrides();
                packages.push(package);
            }
        }

        if packages.is_empty() {
            return Err(Error::unexpected(format!(
                "failed to parse any Mason LSP packages from {}",
                path.display()
            )));
        }

        Ok(Self::from_packages(packages))
    }

    fn from_packages(packages: Vec<MasonPackage>) -> Self {
        let mut lspconfigs = BTreeMap::new();
        let mut package_names = BTreeMap::new();
        let mut binaries = BTreeMap::new();
        let mut ambiguous_bins = std::collections::BTreeSet::new();

        for mut package in packages {
            if !package.is_lsp() {
                continue;
            }

            let _ = package.apply_source_version_overrides();

            let package_name = package.name.clone();

            for binary in package.bin.keys() {
                if let Some(existing) = binaries.get(binary) {
                    if existing != &package_name {
                        ambiguous_bins.insert(binary.clone());
                    }
                } else {
                    binaries.insert(binary.clone(), package_name.clone());
                }
            }

            package_names.insert(package_name, package.clone());

            let Some(lspconfig) = package.neovim.lspconfig.clone() else {
                continue;
            };
            lspconfigs.insert(lspconfig, package);
        }

        for binary in ambiguous_bins {
            binaries.remove(&binary);
        }

        Self {
            lspconfigs,
            package_names,
            binaries,
        }
    }
}

#[derive(Clone, Copy)]
enum MappingOverride {
    Lspconfig(&'static str),
    Package(&'static str),
}

fn mapping_override(config_id: &str) -> Option<MappingOverride> {
    match config_id {
        // Conservative historical aliases that are still common in user config.
        "sumneko_lua" => Some(MappingOverride::Lspconfig("lua_ls")),
        "tsserver" => Some(MappingOverride::Lspconfig("ts_ls")),
        "typescript_language_server" => {
            Some(MappingOverride::Package("typescript-language-server"))
        }
        "volar" => Some(MappingOverride::Lspconfig("vue_ls")),
        _ => None,
    }
}

fn is_lsp_package_value(value: &serde_json::Value) -> bool {
    value
        .get("categories")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|categories| {
            categories
                .iter()
                .any(|category| category.as_str() == Some("LSP"))
        })
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonPackage {
    pub name: String,
    #[serde(default)]
    pub categories: Vec<String>,
    pub source: MasonSource,
    #[serde(default)]
    pub bin: BTreeMap<String, String>,
    #[serde(default)]
    pub share: BTreeMap<String, String>,
    #[serde(default)]
    pub neovim: MasonNeovim,
}

impl MasonPackage {
    fn is_lsp(&self) -> bool {
        self.categories.iter().any(|category| category == "LSP")
    }

    fn apply_source_version_overrides(&mut self) -> Result<()> {
        self.source.apply_version_overrides()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonSource {
    pub id: String,
    #[serde(default)]
    pub extra_packages: Vec<String>,
    #[serde(default)]
    pub asset: Option<OneOrMany<MasonAsset>>,
    #[serde(default)]
    pub download: Option<OneOrMany<MasonDownload>>,
    #[serde(default)]
    pub version_overrides: Vec<MasonVersionOverride>,
}

impl MasonSource {
    #[must_use]
    pub fn assets(&self) -> &[MasonAsset] {
        self.asset.as_ref().map_or(&[], OneOrMany::as_slice)
    }

    #[must_use]
    pub fn downloads(&self) -> &[MasonDownload] {
        self.download.as_ref().map_or(&[], OneOrMany::as_slice)
    }

    fn apply_version_overrides(&mut self) -> Result<()> {
        let version = source_id_version(&self.id)?;
        let override_ = self
            .version_overrides
            .iter()
            .filter(|override_| version_matches_constraint(version, &override_.constraint))
            .min_by(|left, right| {
                compare_versions(
                    constraint_upper_bound(&left.constraint),
                    constraint_upper_bound(&right.constraint),
                )
            })
            .cloned();

        let Some(override_) = override_ else {
            return Ok(());
        };

        self.id = override_.id;
        assign_if_some(&mut self.extra_packages, override_.extra_packages);
        assign_option_if_some(&mut self.asset, override_.asset);
        assign_option_if_some(&mut self.download, override_.download);

        Ok(())
    }
}

fn assign_if_some<T>(target: &mut T, replacement: Option<T>) {
    if let Some(replacement) = replacement {
        *target = replacement;
    }
}

fn assign_option_if_some<T>(target: &mut Option<T>, replacement: Option<T>) {
    if let Some(replacement) = replacement {
        *target = Some(replacement);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonVersionOverride {
    pub constraint: String,
    pub id: String,
    #[serde(default)]
    pub extra_packages: Option<Vec<String>>,
    #[serde(default)]
    pub asset: Option<OneOrMany<MasonAsset>>,
    #[serde(default)]
    pub download: Option<OneOrMany<MasonDownload>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        match self {
            Self::One(value) => std::slice::from_ref(value),
            Self::Many(values) => values,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonAsset {
    #[serde(default)]
    pub target: Option<OneOrMany<String>>,
    pub file: OneOrMany<String>,
    #[serde(default)]
    pub bin: Option<MasonAssetBin>,
    #[serde(default)]
    pub ext: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MasonAssetBin {
    One(String),
    Many(BTreeMap<String, String>),
}

impl MasonAssetBin {
    #[must_use]
    pub fn as_single(&self) -> Option<&str> {
        match self {
            Self::One(value) => Some(value),
            Self::Many(_) => None,
        }
    }

    #[must_use]
    pub fn as_map(&self) -> Option<&BTreeMap<String, String>> {
        match self {
            Self::One(_) => None,
            Self::Many(values) => Some(values),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonDownload {
    #[serde(default)]
    pub target: Option<OneOrMany<String>>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    pub bin: Option<String>,
    pub config: Option<String>,
    #[serde(default)]
    pub man: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct MasonNeovim {
    pub lspconfig: Option<String>,
}

fn source_id_version(source_id: &str) -> Result<&str> {
    source_id
        .strip_prefix("pkg:")
        .and_then(|value| value.rsplit_once('@').map(|(_, version)| version))
        .ok_or_else(|| Error::unexpected(format!("unsupported Mason package source {source_id}")))
}

fn version_matches_constraint(version: &str, constraint: &str) -> bool {
    let Some(upper_bound) = constraint_upper_bound(constraint) else {
        return false;
    };

    compare_versions(Some(version), Some(upper_bound)).is_le()
}

fn constraint_upper_bound(constraint: &str) -> Option<&str> {
    constraint
        .strip_prefix("semver:")
        .and_then(|value| value.strip_prefix("<="))
}

fn compare_versions(left: Option<&str>, right: Option<&str>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_version_strings(left, right),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn compare_version_strings(left: &str, right: &str) -> std::cmp::Ordering {
    let left = tokenize_version(left);
    let right = tokenize_version(right);
    let max_len = left.len().max(right.len());

    for index in 0..max_len {
        let ordering = compare_version_part(left.get(index), right.get(index));
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }

    std::cmp::Ordering::Equal
}

fn tokenize_version(version: &str) -> Vec<VersionPart> {
    let trimmed = version.trim_start_matches('v');
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = None;

    for ch in trimmed.chars() {
        if !ch.is_ascii_alphanumeric() {
            push_version_part(&mut parts, &mut current, &mut current_is_digit);
            continue;
        }

        let is_digit = ch.is_ascii_digit();
        if current_is_digit.is_some_and(|value| value != is_digit) {
            push_version_part(&mut parts, &mut current, &mut current_is_digit);
        }
        current_is_digit = Some(is_digit);
        current.push(ch);
    }

    push_version_part(&mut parts, &mut current, &mut current_is_digit);
    parts
}

fn push_version_part(
    parts: &mut Vec<VersionPart>,
    current: &mut String,
    current_is_digit: &mut Option<bool>,
) {
    if current.is_empty() {
        *current_is_digit = None;
        return;
    }

    if current_is_digit.unwrap_or(false) {
        parts.push(VersionPart::Number(
            current.parse::<u64>().unwrap_or(u64::MAX),
        ));
    } else {
        parts.push(VersionPart::Text(current.to_ascii_lowercase()));
    }

    current.clear();
    *current_is_digit = None;
}

#[derive(Debug)]
enum VersionPart {
    Number(u64),
    Text(String),
}

fn compare_version_part(
    left: Option<&VersionPart>,
    right: Option<&VersionPart>,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(VersionPart::Number(left)), Some(VersionPart::Number(right))) => left.cmp(right),
        (Some(VersionPart::Text(left)), Some(VersionPart::Text(right))) => left.cmp(right),
        (Some(VersionPart::Number(left)), Some(VersionPart::Text(_))) => {
            if *left == 0 {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }
        (Some(VersionPart::Text(_)), Some(VersionPart::Number(right))) => {
            if *right == 0 {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        }
        (Some(part), None) => compare_version_part(Some(part), Some(&VersionPart::Number(0))),
        (None, Some(part)) => compare_version_part(Some(&VersionPart::Number(0)), Some(part)),
        (None, None) => std::cmp::Ordering::Equal,
    }
}
