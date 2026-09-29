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
    pub(super) fn label(&self) -> String {
        format!("{}/{}", self.language, self.server)
    }

    pub(super) fn is_smoke(&self) -> bool {
        self.tier == Some(super::PairTier::Smoke)
    }

    pub(super) fn has_executable_behavior(&self) -> bool {
        matches!(
            self.smoke,
            Some(SmokeDisposition::Queries { .. } | SmokeDisposition::Capabilities { .. })
        ) || matches!(self.lifecycle, Some(LifecycleDisposition::Scenarios { .. }))
    }

    pub(super) fn key(&self) -> PairKey {
        PairKey {
            language: self.language.clone(),
            server: self.server.clone(),
        }
    }
}
