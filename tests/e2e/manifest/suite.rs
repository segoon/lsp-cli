use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct Platform {
    pub(super) os: OperatingSystem,
    pub(super) architecture: Architecture,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum OperatingSystem {
    Linux,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Architecture {
    X86_64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct TestDefaults {
    pub(super) smoke: Timeouts,
    pub(super) lifecycle: Timeouts,
    pub(super) provisioning: ProvisioningDefaults,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct Timeouts {
    pub(super) lsp_timeout_seconds: u64,
    pub(super) deadline_seconds: u64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct ProvisioningDefaults {
    pub(super) deadline_seconds: u64,
}

impl Timeouts {
    pub(super) fn resolve(self, lsp: Option<u64>, deadline: Option<u64>) -> (u64, u64) {
        (
            lsp.unwrap_or(self.lsp_timeout_seconds),
            deadline.unwrap_or(self.deadline_seconds),
        )
    }
}

impl TestDefaults {
    pub(super) fn validate(self) -> Result<(), String> {
        for (label, value) in [("smoke", self.smoke), ("lifecycle", self.lifecycle)] {
            if value.lsp_timeout_seconds == 0 || value.deadline_seconds < value.lsp_timeout_seconds
            {
                return Err(format!(
                    "E2E {label} default deadlines must be positive and ordered"
                ));
            }
        }
        if self.provisioning.deadline_seconds == 0 {
            return Err("E2E provisioning default deadline must be positive".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> TestDefaults {
        TestDefaults {
            smoke: Timeouts {
                lsp_timeout_seconds: 30,
                deadline_seconds: 300,
            },
            lifecycle: Timeouts {
                lsp_timeout_seconds: 20,
                deadline_seconds: 240,
            },
            provisioning: ProvisioningDefaults {
                deadline_seconds: 600,
            },
        }
    }

    #[test]
    fn resolves_timeout_defaults_and_independent_overrides() {
        assert_eq!(defaults().smoke.resolve(None, None), (30, 300));
        assert_eq!(defaults().smoke.resolve(Some(4), None), (4, 300));
        assert_eq!(defaults().smoke.resolve(None, Some(9)), (30, 9));
    }

    #[test]
    fn rejects_invalid_suite_defaults() {
        let mut defaults = defaults();
        defaults.lifecycle.deadline_seconds = 19;

        assert!(defaults.validate().is_err());
    }
}
