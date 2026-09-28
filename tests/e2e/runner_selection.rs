use crate::manifest::Manifest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Suite {
    All,
    Smoke,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Phase {
    All,
    Provision,
    Smoke,
    Lifecycle,
}

#[derive(Debug)]
pub(crate) struct Selection {
    pub(crate) suite: Suite,
    pub(crate) phase: Phase,
    pub(crate) case: Option<String>,
    pub(crate) server: Option<String>,
}

impl Selection {
    pub(crate) fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut suite = None;
        let mut phase = Phase::All;
        let mut case = None;
        let mut server = None;
        let mut args = args;
        while let Some(flag) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("{flag} requires a value"))?;
            match flag.as_str() {
                "--suite" => suite = Some(parse_suite(&value)?),
                "--phase" => phase = parse_phase(&value)?,
                "--case" => case = Some(value),
                "--server" => server = Some(value),
                _ => return Err(format!("unknown E2E runner option {flag:?}")),
            }
        }
        if case.is_some() && server.is_some() {
            return Err("CASE and SERVER cannot be used together".to_string());
        }
        Ok(Self {
            suite: suite.ok_or_else(|| "--suite is required".to_string())?,
            phase,
            case,
            server,
        })
    }

    pub(crate) fn validate(&self, manifest: &Manifest) -> Result<(), String> {
        if let Some(label) = &self.case
            && !manifest.declares_explicit_pair(label)
        {
            return Err(format!("CASE {label:?} is not a declared E2E pair"));
        }
        if let Some(server) = &self.server
            && !manifest.declares_server(server)
        {
            return Err(format!("SERVER {server:?} is not in the E2E inventory"));
        }
        Ok(())
    }

    pub(crate) fn includes_pair(&self, label: &str, server: &str, smoke: bool) -> bool {
        self.case.as_deref().is_none_or(|value| value == label)
            && self.server.as_deref().is_none_or(|value| value == server)
            && (self.suite == Suite::All || smoke)
    }

    pub(crate) fn includes_server(&self, manifest: &Manifest, server: &str) -> bool {
        let selected_server = self
            .case
            .as_deref()
            .and_then(|label| manifest.pair_server(label))
            .or(self.server.as_deref());
        selected_server.is_none_or(|value| value == server)
            && (self.suite == Suite::All || manifest.smoke_servers().contains(server))
    }
}

fn parse_suite(value: &str) -> Result<Suite, String> {
    match value {
        "all" => Ok(Suite::All),
        "smoke" => Ok(Suite::Smoke),
        _ => Err(format!("unknown suite {value:?}; expected all or smoke")),
    }
}

fn parse_phase(value: &str) -> Result<Phase, String> {
    match value {
        "all" => Ok(Phase::All),
        "provision" => Ok(Phase::Provision),
        "smoke" => Ok(Phase::Smoke),
        "lifecycle" => Ok(Phase::Lifecycle),
        _ => Err(format!(
            "unknown phase {value:?}; expected all, provision, smoke, or lifecycle"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Selection, String> {
        Selection::parse(args.iter().map(|value| (*value).to_string()))
    }

    #[test]
    fn parses_targeted_phase() {
        let selection = parse(&[
            "--suite",
            "all",
            "--case",
            "rust/rust_analyzer",
            "--phase",
            "lifecycle",
        ])
        .expect("selection should parse");

        assert_eq!(selection.suite, Suite::All);
        assert_eq!(selection.phase, Phase::Lifecycle);
        assert_eq!(selection.case.as_deref(), Some("rust/rust_analyzer"));
    }

    #[test]
    fn rejects_conflicting_selectors() {
        let error = parse(&[
            "--suite",
            "all",
            "--case",
            "rust/rust_analyzer",
            "--server",
            "rust_analyzer",
        ])
        .expect_err("selectors should conflict");

        assert_eq!(error, "CASE and SERVER cannot be used together");
    }

    #[test]
    fn rejects_unknown_phase() {
        let error =
            parse(&["--suite", "all", "--phase", "fast"]).expect_err("unknown phases should fail");

        assert!(error.contains("expected all, provision, smoke, or lifecycle"));
    }
}
