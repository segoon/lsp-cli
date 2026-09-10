use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use crate::harness::{E2eContext, E2eOutput};
use crate::manifest::{
    ExceptionOutcome, Manifest, QueryKind, RealServerCapabilitiesCase, RealServerCase,
};
use crate::real_server_support::{CaseDeadline, run_isolated_case, run_reported_case};
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

struct CapabilitiesTest<'a> {
    case: RealServerCapabilitiesCase<'a>,
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
        run_reported_case("case", &label, || self.run_inner())
    }

    fn run_inner(&self) -> Result<(), String> {
        let deadline = CaseDeadline::new(self.case.deadline_seconds(), "case");
        run_isolated_case(
            &self.repository.join(self.case.project()),
            self.case.host_programs(),
            &deadline,
            |context| {
                let server = self.case.server_name(self.repository)?;
                let capabilities = self.capabilities(context, &server, deadline.remaining()?)?;
                for command in QUERY_COMMANDS.into_iter().skip(1) {
                    self.run_query(
                        context,
                        &server,
                        &capabilities.capabilities,
                        command,
                        deadline.remaining()?,
                    )?;
                }
                Ok(())
            },
        )
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
        context.record_server_capabilities(&response.capabilities)?;
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
        if matches!(
            command,
            QueryKind::Grep
                | QueryKind::References
                | QueryKind::Callers
                | QueryKind::Callees
                | QueryKind::Definition
                | QueryKind::Declaration
        ) {
            return self.retry_symbol_query_if_empty(context, server, command, deadline, output);
        }
        self.validate_success(command, context, &output)
    }

    // Symbol- and call-hierarchy resolution can transiently lag behind a
    // server's background-indexing-done signal, so an unexpected empty
    // result gets a couple of retries before it's treated as a real failure.
    fn retry_symbol_query_if_empty(
        &self,
        context: &E2eContext,
        server: &str,
        command: QueryKind,
        deadline: Duration,
        mut output: E2eOutput,
    ) -> Result<(), String> {
        const ATTEMPTS: u32 = 3;
        let mut last_error = String::new();
        for attempt in 1..=ATTEMPTS {
            match self.validate_success(command, context, &output) {
                Ok(()) => return Ok(()),
                Err(error) => last_error = error,
            }
            if attempt == ATTEMPTS {
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
            output = run(context, &self.command_args(command, server), deadline)
                .map_err(|error| format!("{command:?} could not complete:\n{error}"))?;
            output.ensure_success()?;
        }
        Err(last_error)
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
        args.push("--download".to_string());
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

impl CapabilitiesTest<'_> {
    fn run(self) -> Result<(), String> {
        let label = self.case.label();
        run_reported_case("capabilities case", &label, || self.run_inner())
    }

    fn run_inner(&self) -> Result<(), String> {
        let deadline = CaseDeadline::new(self.case.deadline_seconds(), "capabilities case");
        run_isolated_case(
            &self.repository.join(self.case.project()),
            self.case.host_programs(),
            &deadline,
            |context| {
                let server = self.case.server_name(self.repository)?;
                let args = [
                    "server-capabilities".to_string(),
                    ".".to_string(),
                    "--lang".to_string(),
                    self.case.language().to_string(),
                    "--lsp".to_string(),
                    server,
                    "--download".to_string(),
                    "--no-detach".to_string(),
                    "--timeout".to_string(),
                    self.case.lsp_timeout_seconds().to_string(),
                    "--json".to_string(),
                ];
                let output = run(context, &args, deadline.remaining()?)?;
                output.ensure_success()?;
                let response: CapabilitiesOutput = output.try_json()?;
                context.record_server_capabilities(&response.capabilities)?;
                if response.capabilities.is_object() && !response.server.command.is_empty() {
                    Ok(())
                } else {
                    Err("server-capabilities returned an invalid semantic payload".to_string())
                }
            },
        )
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

#[test]
#[ignore = "downloads and runs real LSP servers; executed explicitly in CI"]
fn manifest_real_server_smoke_cases() {
    let repository = repository_root();
    let manifest = Manifest::load_validated(repository).expect("E2E manifest should be valid");
    let selected = selected_cases(&manifest);
    assert!(
        manifest.supports_current_platform(),
        "real-server E2E requires {}; current platform is {}/{}",
        manifest.platform_label(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    assert!(
        selected
            .as_ref()
            .is_none_or(|labels| labels.iter().all(|label| manifest.declares_pair(label))),
        "E2E selection contains an undeclared manifest pair"
    );
    if let Some(labels) = &selected {
        for label in labels {
            if let Some(reason) = manifest.exclusion_reason(label) {
                eprintln!("E2E case {label}: reviewed exclusion: {reason}");
            }
        }
    }
    let cases = manifest
        .real_server_smoke_cases()
        .filter(|case| {
            selected.as_ref().map_or_else(
                || manifest.is_preferred_pair(&case.label()),
                |expected| expected.contains(&case.label()),
            )
        })
        .collect::<Vec<_>>();
    let mut failures = cases
        .into_iter()
        .filter_map(|case| RealServerTest::new(case, repository).run().err())
        .collect::<Vec<_>>();
    failures.extend(
        manifest
            .real_server_capabilities_cases()
            .filter(|case| {
                selected
                    .as_ref()
                    .is_some_and(|expected| expected.contains(&case.label()))
            })
            .filter_map(|case| CapabilitiesTest { case, repository }.run().err()),
    );
    assert!(
        failures.is_empty(),
        "real-server E2E failures:\n{}",
        failures.join("\n\n")
    );
}

fn selected_cases(manifest: &Manifest) -> Option<BTreeSet<String>> {
    let single = std::env::var("E2E_CASE").ok();
    let batch = std::env::var("E2E_CASES").ok();
    assert!(
        single.is_none() || batch.is_none(),
        "E2E_CASE and E2E_CASES cannot be used together"
    );
    if let Some(label) = single {
        return Some(BTreeSet::from([label]));
    }
    batch.map(|value| {
        let labels = value
            .split(',')
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        assert!(
            !labels.is_empty(),
            "E2E_CASES must select at least one pair"
        );
        assert!(
            labels
                .iter()
                .all(|label| manifest.declares_explicit_pair(label)),
            "E2E_CASES may contain only explicit manifest pairs"
        );
        labels
    })
}
