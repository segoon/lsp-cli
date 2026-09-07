use std::path::Path;

use super::*;

impl LanguageFile {
    pub(super) fn into_parts(
        self,
        case_id: &str,
        path: &Path,
    ) -> Result<(LanguageCase, Vec<PairCase>), String> {
        if self.language.id != case_id {
            return Err(format!(
                "E2E case filename {case_id:?} does not match language ID {:?} in {}",
                self.language.id,
                path.display()
            ));
        }
        if let Some(pair) = self
            .pairs
            .iter()
            .find(|pair| pair.language != self.language.id)
        {
            return Err(format!(
                "E2E case {} for language {:?} contains pair for language {:?}",
                path.display(),
                self.language.id,
                pair.language
            ));
        }
        Ok((self.language, self.pairs))
    }
}

impl PairCase {
    pub(super) fn key(&self) -> PairKey {
        PairKey {
            language: self.language.clone(),
            server: self.server.clone(),
        }
    }
}
