use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::harness::{E2eContext, E2eOutput};
use crate::manifest::{ExceptionOutcome, Manifest, ProvisionMethod, QueryKind, RealServerCase};
use crate::repository_root;

const QUERY_COMMANDS: [QueryKind; 12] = [
    QueryKind::ServerCapabilities,
    QueryKind::Diagnostics,
    QueryKind::Format,
    QueryKind::Grep,
    QueryKind::ListSymbols,
    QueryKind::ListFunctions,
    QueryKind::References,
    QueryKind::Callers,
    QueryKind::Callees,
    QueryKind::Definition,
    QueryKind::Declaration,
    QueryKind::BuildIndex,
];

struct RealServerTest<'a> {
    case: RealServerCase<'a>,
    repository: &'a Path,
}

#[derive(Deserialize)]
struct CapabilitiesOutput {
    capabilities: Value,
    server: ServerOutput,
}

#[derive(Deserialize)]
struct ServerOutput {
    command: Vec<String>,
}

impl<'a> RealServerTest<'a> {
    fn new(case: RealServerCase<'a>, repository: &'a Path) -> Self {
        Self { case, repository }
    }

    fn run(self) -> Result<(), String> {
        let label = self.case.label();
        eprintln!("E2E case {label}: started");
        let result = self
            .run_inner()
            .map_err(|error| format!("E2E case {label} failed:\n{error}"));
        eprintln!("E2E case {label}: finished");
        result
    }

    fn run_inner(&self) -> Result<(), String> {
        let started = Instant::now();
        let deadline = Duration::from_secs(self.case.deadline_seconds());
        let context = E2eContext::new()
            .map_err(|error| format!("failed to create an isolated E2E context: {error}"))?;
        context.copy_project(&self.repository.join(self.case.project()))?;
        for (name, resolver) in self.case.host_programs() {
            context.stage_host_program(name, resolver, remaining(started, deadline)?)?;
        }
        let server = self.case.server_name(self.repository)?;
        let capabilities = self.capabilities(&context, &server, remaining(started, deadline)?)?;
        for command in QUERY_COMMANDS.into_iter().skip(1) {
            self.run_query(
                &context,
                &server,
                &capabilities.capabilities,
                command,
                remaining(started, deadline)?,
            )?;
        }
        Ok(())
    }

    fn capabilities(
        &self,
        context: &E2eContext,
        server: &str,
        deadline: Duration,
    ) -> Result<CapabilitiesOutput, String> {
        let output = run(
            context,
            &self.command_args(QueryKind::ServerCapabilities, server),
            deadline,
        )?;
        output.ensure_success()?;
        let response: CapabilitiesOutput = output.try_json()?;
        let program = response
            .server
            .command
            .first()
            .ok_or_else(|| "selected server reported an empty command".to_string())?;
        if !Path::new(program).starts_with(context.home()) {
            return Err(format!(
                "downloaded server program {program} is outside isolated home {}",
                context.home().display()
            ));
        }
        Ok(response)
    }

    fn run_query(
        &self,
        context: &E2eContext,
        server: &str,
        capabilities: &Value,
        command: QueryKind,
        deadline: Duration,
    ) -> Result<(), String> {
        let output = run(context, &self.command_args(command, server), deadline)
            .map_err(|error| format!("{command:?} could not complete:\n{error}"))?;
        if capability_path(command).is_some_and(|path| !supports(capabilities, path)) {
            return validate_unsupported(command, &output);
        }
        if let Some((outcome, message, reason)) = self.case.exception(command) {
            return validate_exception(command, outcome, message, reason, &output);
        }
        output.ensure_success()?;
        self.validate_success(command, context, &output)
    }

    fn command_args(&self, command: QueryKind, server: &str) -> Vec<String> {
        let mut args = query_prefix(
            command,
            self.case.symbol_query(),
            self.case.callable_query(),
            self.case.format_file(),
        );
        args.extend([
            "--lang".to_string(),
            self.case.language().to_string(),
            "--lsp".to_string(),
            server.to_string(),
        ]);
        if matches!(self.case.provision_method(), ProvisionMethod::Download) {
            args.push("--download".to_string());
        }
        args.extend([
            "--no-detach".to_string(),
            "--timeout".to_string(),
            self.case.lsp_timeout_seconds().to_string(),
        ]);
        if !matches!(command, QueryKind::Format | QueryKind::BuildIndex) {
            args.push("--json".to_string());
        }
        args
    }

    fn validate_success(
        &self,
        command: QueryKind,
        context: &E2eContext,
        output: &E2eOutput,
    ) -> Result<(), String> {
        match command {
            QueryKind::Diagnostics => validate_diagnostics(self.case.language(), output),
            QueryKind::Format => validate_format(context, self.case.format_file(), output),
            QueryKind::Grep
            | QueryKind::ListSymbols
            | QueryKind::ListFunctions
            | QueryKind::References
            | QueryKind::Callers
            | QueryKind::Callees
            | QueryKind::Definition
            | QueryKind::Declaration => {
                validate_matches(command, self.case.expected_names(), output)
            }
            QueryKind::BuildIndex => Ok(()),
            QueryKind::ServerCapabilities => {
                Err("capabilities must be validated before query execution".to_string())
            }
        }
    }
}

fn query_prefix(
    command: QueryKind,
    symbol: &str,
    callable: &str,
    format_file: &Path,
) -> Vec<String> {
    let values: Vec<&str> = match command {
        QueryKind::ServerCapabilities => vec!["server-capabilities", "."],
        QueryKind::Diagnostics => vec!["diagnostics", "."],
        QueryKind::Format => vec![
            "format",
            format_file.to_str().expect("validated UTF-8 fixture path"),
            "--stdout",
        ],
        QueryKind::Grep => vec!["grep", symbol, "."],
        QueryKind::ListSymbols => vec!["list-symbols", "."],
        QueryKind::ListFunctions => vec!["list-functions", "."],
        QueryKind::References => vec!["references", callable, "."],
        QueryKind::Callers => vec!["callers", callable, "."],
        QueryKind::Callees => vec!["callees", callable, "."],
        QueryKind::Definition => vec!["definition", callable, "."],
        QueryKind::Declaration => vec!["declaration", callable, "."],
        QueryKind::BuildIndex => vec!["build-index", "."],
    };
    values.into_iter().map(str::to_string).collect()
}

fn capability_path(command: QueryKind) -> Option<&'static [&'static str]> {
    match command {
        QueryKind::Grep => Some(&["workspaceSymbolProvider"]),
        QueryKind::ListSymbols | QueryKind::ListFunctions => Some(&["documentSymbolProvider"]),
        QueryKind::References => Some(&["referencesProvider"]),
        QueryKind::Callers | QueryKind::Callees => Some(&["callHierarchyProvider"]),
        QueryKind::Definition => Some(&["definitionProvider"]),
        QueryKind::Declaration => Some(&["declarationProvider"]),
        QueryKind::Format => Some(&["documentFormattingProvider"]),
        QueryKind::ServerCapabilities | QueryKind::Diagnostics | QueryKind::BuildIndex => None,
    }
}

fn supports(capabilities: &Value, path: &[&str]) -> bool {
    let value = path
        .iter()
        .try_fold(capabilities, |value, part| value.get(*part));
    !matches!(value, None | Some(Value::Bool(false) | Value::Null))
}

fn validate_unsupported(command: QueryKind, output: &E2eOutput) -> Result<(), String> {
    if output.ensure_success().is_ok() {
        return Err(format!(
            "{command:?} succeeded without advertising its capability"
        ));
    }
    let expected = match command {
        QueryKind::Grep => "does not support workspace/symbol",
        QueryKind::ListSymbols => "does not support list-symbols",
        QueryKind::ListFunctions => "does not support list-functions",
        QueryKind::References => "does not support textDocument/references",
        QueryKind::Callers | QueryKind::Callees => "does not support call hierarchy",
        QueryKind::Definition => "does not support textDocument/definition",
        QueryKind::Declaration => "does not support textDocument/declaration",
        QueryKind::Format => "does not support format",
        _ => {
            return Err(format!(
                "{command:?} has no unsupported-capability contract"
            ));
        }
    };
    if output.stderr_text().contains(expected) {
        Ok(())
    } else {
        Err(format!(
            "{command:?} did not report {expected:?}: {}",
            output.stderr_text()
        ))
    }
}

fn validate_exception(
    command: QueryKind,
    outcome: ExceptionOutcome,
    message: Option<&str>,
    reason: &str,
    output: &E2eOutput,
) -> Result<(), String> {
    match outcome {
        ExceptionOutcome::Failure if output.ensure_success().is_err() => {
            let expected = message
                .ok_or_else(|| format!("{command:?} failure exception omitted its message"))?;
            if output.stderr_text().contains(expected) {
                Ok(())
            } else {
                Err(format!(
                    "{command:?} did not report {expected:?} ({reason}): {}",
                    output.stderr_text()
                ))
            }
        }
        ExceptionOutcome::EmptyMatches => {
            output.ensure_success()?;
            let value: Value = output.try_json()?;
            if value
                .get("matches")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
            {
                Ok(())
            } else {
                Err(format!("{command:?} expected no matches ({reason})"))
            }
        }
        ExceptionOutcome::Failure => Err(format!("{command:?} unexpectedly succeeded ({reason})")),
    }
}

fn validate_diagnostics(language: &str, output: &E2eOutput) -> Result<(), String> {
    let value: Value = output.try_json()?;
    let detected = value
        .get("detected")
        .and_then(Value::as_array)
        .ok_or_else(|| "diagnostics omitted detected languages".to_string())?;
    if detected.iter().any(|item| item == language)
        && value.get("diagnostics").is_some_and(Value::is_array)
    {
        Ok(())
    } else {
        Err("diagnostics returned an invalid semantic payload".to_string())
    }
}

fn validate_format(context: &E2eContext, file: &Path, output: &E2eOutput) -> Result<(), String> {
    let source = std::fs::read_to_string(context.workspace().join(file))
        .map_err(|error| format!("failed to reread format fixture: {error}"))?;
    if output.stdout_text().is_empty() || source.is_empty() {
        Err("format --stdout returned empty source".to_string())
    } else {
        Ok(())
    }
}

fn validate_matches(
    command: QueryKind,
    expected: &[String],
    output: &E2eOutput,
) -> Result<(), String> {
    let value: Value = output.try_json()?;
    let matches = value
        .get("matches")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{command:?} omitted matches"))?;
    if matches.is_empty() {
        return Err(format!("{command:?} returned no semantic matches"));
    }
    if command != QueryKind::ListSymbols {
        return Ok(());
    }
    let actual = matches
        .iter()
        .filter_map(|item| item.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let missing = expected
        .iter()
        .filter(|name| !actual.contains(name.as_str()))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "list-symbols missed {missing:?}; returned {actual:?}"
        ))
    }
}

fn run(context: &E2eContext, args: &[String], deadline: Duration) -> Result<E2eOutput, String> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    context.try_run_with_deadline(&refs, deadline)
}

fn remaining(started: Instant, deadline: Duration) -> Result<Duration, String> {
    let remaining = deadline.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        Err(format!(
            "case exceeded its overall deadline of {deadline:?}"
        ))
    } else {
        Ok(remaining)
    }
}

#[test]
#[ignore = "downloads and runs real LSP servers; executed explicitly in CI"]
fn manifest_real_server_smoke_cases() {
    let repository = repository_root();
    let manifest = Manifest::load_validated(repository).expect("E2E manifest should be valid");
    let selected = std::env::var("E2E_CASE").ok();
    assert!(
        selected
            .as_deref()
            .is_none_or(|label| manifest.declares_pair(label)),
        "E2E_CASE {:?} does not select a declared manifest pair",
        selected.as_deref().unwrap_or_default()
    );
    let cases = manifest
        .real_server_smoke_cases()
        .filter(|case| {
            selected
                .as_ref()
                .is_none_or(|expected| case.label() == *expected)
        })
        .collect::<Vec<_>>();
    let failures = cases
        .into_iter()
        .filter_map(|case| RealServerTest::new(case, repository).run().err())
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "real-server E2E failures:\n{}",
        failures.join("\n\n")
    );
}
