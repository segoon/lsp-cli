use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FailureStage {
    Setup,
    Provisioning,
    Capabilities,
    Query,
    Lifecycle,
    Cleanup,
}

impl FailureStage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Setup => "setup",
            Self::Provisioning => "provisioning",
            Self::Capabilities => "capabilities",
            Self::Query => "query",
            Self::Lifecycle => "lifecycle",
            Self::Cleanup => "cleanup",
        }
    }
}

impl fmt::Display for FailureStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
