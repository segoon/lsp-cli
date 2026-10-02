use crate::manifest::Manifest;
use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Run,
    ListWork,
    MergeResults(PathBuf),
}

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
    pub(crate) action: Action,
    pub(crate) suite: Suite,
    pub(crate) phase: Phase,
    pub(crate) case: Option<String>,
    pub(crate) server: Option<String>,
    pub(crate) quiet_summary: bool,
    pub(crate) defer_failures: bool,
}

impl Selection {
    pub(crate) fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut suite = None;
        let mut phase = Phase::All;
        let mut case = None;
        let mut server = None;
        let mut action = Action::Run;
        let mut quiet_summary = false;
        let mut defer_failures = false;
        let mut args = args;
        while let Some(flag) = args.next() {
            if flag == "--list-work" {
                set_action(&mut action, Action::ListWork)?;
                continue;
            }
            if flag == "--quiet-summary" {
                quiet_summary = true;
                continue;
            }
            if flag == "--defer-failures" {
                defer_failures = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{flag} requires a value"))?;
            match flag.as_str() {
                "--suite" => suite = Some(parse_suite(&value)?),
                "--phase" => phase = parse_phase(&value)?,
                "--case" => case = Some(value),
                "--server" => server = Some(value),
                "--merge-results" => {
                    set_action(&mut action, Action::MergeResults(PathBuf::from(value)))?;
                }
                _ => return Err(format!("unknown E2E runner option {flag:?}")),
            }
        }
        if case.is_some() && server.is_some() {
            return Err("CASE and SERVER cannot be used together".to_string());
        }
        Ok(Self {
            action,
            suite: suite.ok_or_else(|| "--suite is required".to_string())?,
            phase,
            case,
            server,
            quiet_summary,
            defer_failures,
        })
    }

    pub(crate) fn validate(&self, manifest: &Manifest) -> Result<(), String> {
        if let Some(label) = &self.case
            && !manifest.declares_explicit_pair(label)
        {
            return Err(format!("CASE {label:?} is not a declared E2E pair"));
        }
        if let Some(server) = &self.server {
            if selected_servers(server).next().is_none() {
                return Err("SERVER requires at least one server ID".to_string());
            }
            for selected in selected_servers(server) {
                if !manifest.declares_server(selected) {
                    return Err(format!("SERVER {selected:?} is not in the E2E inventory"));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn includes_pair(&self, label: &str, server: &str, smoke: bool) -> bool {
        self.case.as_deref().is_none_or(|value| value == label)
            && self
                .server
                .as_deref()
                .is_none_or(|values| selected_servers(values).any(|value| value == server))
            && (self.suite == Suite::All || smoke)
    }

    pub(crate) fn includes_server(&self, manifest: &Manifest, server: &str) -> bool {
        let selected = self
            .case
            .as_deref()
            .and_then(|label| manifest.pair_server(label))
            .is_none_or(|value| value == server)
            && self
                .server
                .as_deref()
                .is_none_or(|values| selected_servers(values).any(|value| value == server));
        selected && (self.suite == Suite::All || manifest.smoke_servers().contains(server))
    }
}

fn selected_servers(value: &str) -> impl Iterator<Item = &str> {
    value.split(',').filter(|server| !server.is_empty())
}

fn set_action(current: &mut Action, requested: Action) -> Result<(), String> {
    if *current != Action::Run {
        return Err("only one E2E runner action may be selected".to_string());
    }
    *current = requested;
    Ok(())
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
        assert_eq!(selection.action, Action::Run);
    }

    #[test]
    fn accepts_multiple_servers() {
        let selection = parse(&["--suite", "all", "--server", "pyright,ruff"])
            .expect("server list should parse");

        assert!(
            selection
                .server
                .as_deref()
                .is_some_and(|value| value == "pyright,ruff")
        );
    }

    #[test]
    fn parses_internal_deferred_failure_mode() {
        let selection = parse(&["--suite", "all", "--defer-failures"])
            .expect("deferred failure mode should parse");

        assert!(selection.defer_failures);
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

    #[test]
    fn parses_internal_actions_and_quiet_output() {
        let selection = parse(&[
            "--suite",
            "smoke",
            "--phase",
            "smoke",
            "--list-work",
            "--quiet-summary",
        ])
        .expect("internal options should parse");

        assert_eq!(selection.action, Action::ListWork);
        assert!(selection.quiet_summary);
    }

    #[test]
    fn rejects_multiple_internal_actions() {
        let error = parse(&[
            "--suite",
            "all",
            "--list-work",
            "--merge-results",
            "results",
        ])
        .expect_err("actions should be exclusive");

        assert_eq!(error, "only one E2E runner action may be selected");
    }
}
