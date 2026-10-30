use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::Path;

use super::FiletypeConfig;
use crate::error::{Error, Result};

#[derive(Clone, Debug, Default)]
pub struct DocumentLanguages {
    filetypes: Vec<FiletypeConfig>,
}

impl DocumentLanguages {
    pub fn new(filetypes: &[FiletypeConfig], allowed: &BTreeSet<String>) -> Self {
        Self {
            filetypes: filetypes
                .iter()
                .filter(|filetype| allowed.contains(&filetype.id))
                .cloned()
                .collect(),
        }
    }

    pub fn resolve(&self, path: &Path) -> Result<&str> {
        let document = FiletypePath::new(path);
        let candidates = self
            .filetypes
            .iter()
            .filter(|filetype| {
                document
                    .as_ref()
                    .is_some_and(|document| document.matches(filetype))
            })
            .collect::<Vec<_>>();
        let ids = candidates
            .iter()
            .map(|filetype| filetype.language_id.as_deref().unwrap_or(&filetype.id))
            .collect::<BTreeSet<_>>();
        if ids.len() == 1
            && let Some(id) = ids.first()
        {
            // Several detection aliases may legitimately identify the same protocol language.
            return Ok(id);
        }
        if candidates.is_empty() {
            return Err(Error::detection(format!(
                "cannot determine the document language for {}: no filetype matches for the selected server and language",
                path.display()
            )));
        }
        Err(Error::detection(format!(
            "cannot determine the document language for {}: matching filetypes {} use different language IDs; select one with --lang",
            path.display(),
            candidates
                .iter()
                .map(|filetype| filetype.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }
}

// Normalize once per document, even when detection checks the entire filetype catalog.
pub struct FiletypePath<'a> {
    name: Cow<'a, str>,
    extension: Option<String>,
}

impl<'a> FiletypePath<'a> {
    pub fn new(path: &'a Path) -> Option<Self> {
        Some(Self {
            name: path.file_name()?.to_string_lossy(),
            extension: path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase),
        })
    }

    pub fn matches(&self, filetype: &FiletypeConfig) -> bool {
        self.extension.as_deref().is_some_and(|extension| {
            filetype
                .extensions
                .iter()
                .any(|candidate| candidate == extension)
        }) || filetype
            .patterns
            .iter()
            .any(|pattern| pattern.is_match(&self.name))
    }
}

#[cfg(test)]
mod tests;
