#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MasonPlatform {
    full: String,
    family: String,
    os: String,
}

impl MasonPlatform {
    pub fn detect() -> Result<Self, String> {
        let os = match std::env::consts::OS {
            "linux" => "linux",
            "macos" => "darwin",
            "windows" => "win",
            other => {
                return Err(format!(
                    "cannot install LSP servers automatically on unsupported platform {other}"
                ));
            }
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            other => {
                return Err(format!(
                    "cannot install LSP servers automatically on unsupported architecture {other}"
                ));
            }
        };

        let full = match os {
            "linux" => format!("{os}_{arch}_{}", target_libc()),
            _ => format!("{os}_{arch}"),
        };
        let family = format!("{os}_{arch}");

        Ok(Self {
            full,
            family,
            os: os.to_string(),
        })
    }

    #[must_use]
    pub fn matches(&self, target: &str) -> bool {
        target == self.full || target == self.family || target == self.os
    }
}

#[cfg(target_env = "musl")]
fn target_libc() -> &'static str {
    "musl"
}

#[cfg(not(target_env = "musl"))]
fn target_libc() -> &'static str {
    "gnu"
}

#[cfg(test)]
mod tests {
    use super::MasonPlatform;

    #[test]
    fn platform_matches_exact_and_family_targets() {
        let platform = MasonPlatform {
            full: "linux_x64_gnu".to_string(),
            family: "linux_x64".to_string(),
            os: "linux".to_string(),
        };

        assert!(platform.matches("linux_x64_gnu"));
        assert!(platform.matches("linux_x64"));
        assert!(platform.matches("linux"));
        assert!(!platform.matches("darwin_x64"));
    }
}
