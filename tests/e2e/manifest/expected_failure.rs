use std::collections::BTreeSet;

use serde::Deserialize;

use super::{LifecycleDisposition, Manifest, SmokeDisposition, require_text};
use crate::failure_stage::FailureStage;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct ExpectedFailure {
    pub(super) case: String,
    stage: Option<FailureStage>,
    diagnostic_contains: Option<String>,
    reason: String,
}

impl Manifest {
    pub(crate) fn expected_failures(&self) -> impl Iterator<Item = &ExpectedFailure> {
        self.expected_failures.iter()
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
            if let Some(diagnostic) = &failure.diagnostic_contains {
                require_text(
                    diagnostic,
                    &format!("expected failure {} diagnostic", failure.case),
                )?;
            }
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

impl ExpectedFailure {
    pub(crate) fn case(&self) -> &str {
        &self.case
    }

    pub(crate) fn matches(
        &self,
        stage: Option<FailureStage>,
        has_additional_stages: bool,
        diagnostic: &str,
    ) -> bool {
        !has_additional_stages
            && self.stage.is_none_or(|expected| Some(expected) == stage)
            && self
                .diagnostic_contains
                .as_deref()
                .is_none_or(|expected| diagnostic.contains(expected))
    }

    pub(crate) fn expectation(&self) -> String {
        let stage = self.stage.map(|value| format!("stage {value}"));
        let diagnostic = self
            .diagnostic_contains
            .as_ref()
            .map(|value| format!("diagnostic containing {value:?}"));
        [
            stage,
            diagnostic,
            Some("no additional failure stages".to_string()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" and ")
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
                stage: None,
                diagnostic_contains: None,
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

        let empty = validation_error("empty diagnostics should fail", |manifest| {
            manifest.expected_failures[0].diagnostic_contains = Some(" ".to_string());
        });
        assert!(empty.contains("diagnostic") && empty.contains("must be non-empty"));
    }

    #[test]
    fn matches_only_the_declared_stage_and_diagnostic() {
        let failure = ExpectedFailure {
            case: "capabilities/rust/example".to_string(),
            stage: Some(FailureStage::Capabilities),
            diagnostic_contains: Some("initialize failed".to_string()),
            reason: "known failure".to_string(),
        };

        assert!(failure.matches(
            Some(FailureStage::Capabilities),
            false,
            "server initialize failed"
        ));
        assert!(!failure.matches(
            Some(FailureStage::Cleanup),
            false,
            "server initialize failed"
        ));
        assert!(!failure.matches(
            Some(FailureStage::Capabilities),
            false,
            "registry unavailable"
        ));
        assert!(!failure.matches(
            Some(FailureStage::Capabilities),
            true,
            "server initialize failed"
        ));
        assert_eq!(
            failure.expectation(),
            "stage capabilities and diagnostic containing \"initialize failed\" and no additional failure stages"
        );
    }
}
