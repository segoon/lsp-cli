use std::collections::BTreeSet;

use serde::Deserialize;

use super::{LifecycleDisposition, Manifest, SmokeDisposition, require_text};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct ExpectedFailure {
    pub(super) case: String,
    reason: String,
}

impl Manifest {
    pub(crate) fn expected_failure_keys(&self) -> impl Iterator<Item = &str> {
        self.expected_failures
            .iter()
            .map(|failure| failure.case.as_str())
    }

    pub(super) fn validate_expected_failures(&self) -> Result<(), String> {
        let mut available = BTreeSet::new();
        for server in &self.servers {
            if server.is_downloadable() {
                available.insert(format!("provisioning/{}", server.id));
            }
            if server.capability_timeouts(self.defaults.smoke).is_some() {
                available.insert(format!(
                    "capabilities/{}/{}",
                    server.owner_language, server.id
                ));
            }
        }
        for pair in &self.pairs {
            let label = pair.label();
            match pair.smoke {
                Some(SmokeDisposition::Queries { .. }) => {
                    available.insert(format!("smoke/{label}"));
                }
                Some(SmokeDisposition::Capabilities { .. }) => {
                    available.insert(format!("capabilities/{label}"));
                }
                Some(SmokeDisposition::Excluded { .. }) | None => {}
            }
            if matches!(pair.lifecycle, Some(LifecycleDisposition::Scenarios { .. })) {
                available.insert(format!("lifecycle/{label}"));
            }
        }

        let mut declared = BTreeSet::new();
        for failure in &self.expected_failures {
            require_text(
                &failure.reason,
                &format!("expected failure {}", failure.case),
            )?;
            if !available.contains(&failure.case) {
                return Err(format!(
                    "expected failure {:?} is not an executable E2E case",
                    failure.case
                ));
            }
            if !declared.insert(&failure.case) {
                return Err(format!("duplicate expected failure {:?}", failure.case));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::validation_error;
    use super::*;

    #[test]
    fn rejects_unknown_and_duplicate_cases() {
        let unknown = validation_error("unknown expected failures should fail", |manifest| {
            manifest.expected_failures.push(ExpectedFailure {
                case: "provisioning/not-a-server".to_string(),
                reason: "known failure".to_string(),
            });
        });
        assert!(unknown.contains("is not an executable E2E case"));

        let duplicate = validation_error("duplicate expected failures should fail", |manifest| {
            let failure = manifest
                .expected_failures
                .first()
                .expect("manifest should declare expected failures")
                .clone();
            manifest.expected_failures.push(failure);
        });
        assert!(duplicate.contains("duplicate expected failure"));
    }
}
