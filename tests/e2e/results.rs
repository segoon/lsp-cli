use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

const RESULTS_OUTPUT_ENV: &str = "E2E_RESULTS_OUTPUT";
const SUMMARY_OUTPUT_ENV: &str = "E2E_RESULTS_SUMMARY_OUTPUT";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FailureStage {
    Setup,
    Provisioning,
    Capabilities,
    Query,
    Lifecycle,
    Cleanup,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CaseKind {
    Smoke,
    Capabilities,
    Lifecycle,
    Provisioning,
}

impl CaseKind {
    pub(crate) fn diagnostic_label(self) -> &'static str {
        match self {
            Self::Smoke => "case",
            Self::Capabilities => "capabilities case",
            Self::Lifecycle => "lifecycle",
            Self::Provisioning => "provisioning",
        }
    }
}

impl fmt::Display for FailureStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Setup => "setup",
            Self::Provisioning => "provisioning",
            Self::Capabilities => "capabilities",
            Self::Query => "query",
            Self::Lifecycle => "lifecycle",
            Self::Cleanup => "cleanup",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug)]
pub(crate) struct E2eFailure {
    primary_stage: FailureStage,
    additional_stages: Vec<FailureStage>,
    message: String,
}

impl E2eFailure {
    pub(crate) fn new(stage: FailureStage, message: impl Into<String>) -> Self {
        Self {
            primary_stage: stage,
            additional_stages: Vec::new(),
            message: message.into(),
        }
    }

    pub(crate) fn with_additional(
        mut self,
        stage: FailureStage,
        message: impl fmt::Display,
    ) -> Self {
        if !self.additional_stages.contains(&stage) {
            self.additional_stages.push(stage);
            self.additional_stages.sort();
        }
        self.message.push_str(&format!("\n{message}"));
        self
    }

    pub(crate) fn with_additional_detail(mut self, detail: impl fmt::Display) -> Self {
        self.message.push_str(&format!("\n{detail}"));
        self
    }

    pub(crate) fn render(&self) -> String {
        let mut stages = format!("E2E failure stage: {}", self.primary_stage);
        if !self.additional_stages.is_empty() {
            let additional = self
                .additional_stages
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            stages.push_str(&format!("\nE2E additional failure stages: {additional}"));
        }
        format!("{stages}\n{}", self.message)
    }
}

pub(crate) type E2eResult<T = ()> = Result<T, E2eFailure>;

pub(crate) trait AtStage<T> {
    fn at_stage(self, stage: FailureStage) -> E2eResult<T>;
}

impl<T> AtStage<T> for Result<T, String> {
    fn at_stage(self, stage: FailureStage) -> E2eResult<T> {
        self.map_err(|message| E2eFailure::new(stage, message))
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Outcome {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct CaseResult {
    id: String,
    kind: CaseKind,
    outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_stage: Option<FailureStage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    additional_failure_stages: Vec<FailureStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl CaseResult {
    fn from_result(kind: CaseKind, id: &str, result: &E2eResult) -> Self {
        match result {
            Ok(()) => Self {
                id: id.to_string(),
                kind,
                outcome: Outcome::Passed,
                failure_stage: None,
                additional_failure_stages: Vec::new(),
                message: None,
            },
            Err(failure) => Self {
                id: id.to_string(),
                kind,
                outcome: Outcome::Failed,
                failure_stage: Some(failure.primary_stage),
                additional_failure_stages: failure.additional_stages.clone(),
                message: Some(failure.message.clone()),
            },
        }
    }
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    cases: Vec<CaseResult>,
    summary: Summary,
}

#[derive(Serialize)]
struct Summary {
    passed: usize,
    failed: usize,
    failures_by_stage: BTreeMap<FailureStage, usize>,
}

static RESULTS: OnceLock<Mutex<Vec<CaseResult>>> = OnceLock::new();

pub(crate) fn record_case(kind: CaseKind, id: &str, result: &E2eResult) -> Result<(), String> {
    let results_path = std::env::var_os(RESULTS_OUTPUT_ENV);
    let summary_path = std::env::var_os(SUMMARY_OUTPUT_ENV);
    if results_path.is_none() && summary_path.is_none() {
        return Ok(());
    }

    let results = RESULTS.get_or_init(|| Mutex::new(Vec::new()));
    let mut results = results
        .lock()
        .map_err(|_poisoned| "E2E result collector lock is poisoned".to_string())?;
    let item = CaseResult::from_result(kind, id, result);
    if let Some(existing) = results
        .iter_mut()
        .find(|existing| existing.kind == item.kind && existing.id == item.id)
    {
        *existing = item;
    } else {
        results.push(item);
    }
    results.sort();
    let report = build_report(&results);

    if let Some(path) = results_path {
        write_file(
            Path::new(&path),
            &serde_json::to_vec_pretty(&report)
                .map_err(|error| format!("failed to serialize E2E results: {error}"))?,
        )?;
    }
    if let Some(path) = summary_path {
        write_file(Path::new(&path), render_summary(&report).as_bytes())?;
    }
    Ok(())
}

fn build_report(cases: &[CaseResult]) -> Report {
    let mut failures_by_stage = BTreeMap::new();
    for case in cases {
        if let Some(stage) = case.failure_stage {
            *failures_by_stage.entry(stage).or_insert(0) += 1;
        }
        for stage in &case.additional_failure_stages {
            *failures_by_stage.entry(*stage).or_insert(0) += 1;
        }
    }
    Report {
        schema_version: 1,
        cases: cases.to_vec(),
        summary: Summary {
            passed: cases
                .iter()
                .filter(|case| case.outcome == Outcome::Passed)
                .count(),
            failed: cases
                .iter()
                .filter(|case| case.outcome == Outcome::Failed)
                .count(),
            failures_by_stage,
        },
    }
}

fn render_summary(report: &Report) -> String {
    let mut output = format!(
        "## E2E shard results\n\nPassed: {}; failed: {}.\n",
        report.summary.passed, report.summary.failed
    );
    if !report.summary.failures_by_stage.is_empty() {
        output.push_str("\n| Failure stage | Count |\n| --- | ---: |\n");
        for (stage, count) in &report.summary.failures_by_stage {
            output.push_str(&format!("| `{stage}` | {count} |\n"));
        }
    }
    output
}

fn write_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, contents).map_err(|error| {
        format!(
            "failed to write E2E result file {}: {error}",
            temporary.display()
        )
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        format!(
            "failed to publish E2E result file {}: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passed(id: &str) -> CaseResult {
        CaseResult::from_result(CaseKind::Capabilities, id, &Ok(()))
    }

    fn failed(id: &str, stage: FailureStage) -> CaseResult {
        CaseResult::from_result(
            CaseKind::Capabilities,
            id,
            &Err(E2eFailure::new(stage, "diagnostic")),
        )
    }

    #[test]
    fn report_counts_and_sorts_failure_stages() {
        let report = build_report(&[
            passed("go/gopls"),
            failed("yaml/yamlls", FailureStage::Capabilities),
            failed("asm/asm-lsp", FailureStage::Provisioning),
        ]);

        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.failed, 2);
        assert_eq!(
            render_summary(&report),
            "## E2E shard results\n\nPassed: 1; failed: 2.\n\n\
             | Failure stage | Count |\n| --- | ---: |\n\
             | `provisioning` | 1 |\n| `capabilities` | 1 |\n"
        );
    }

    #[test]
    fn failure_renders_primary_and_cleanup_stages() {
        let failure = E2eFailure::new(FailureStage::Query, "query failed")
            .with_additional(FailureStage::Cleanup, "cleanup failed");

        assert_eq!(
            failure.render(),
            "E2E failure stage: query\nE2E additional failure stages: cleanup\nquery failed\ncleanup failed"
        );
    }

    #[test]
    fn json_omits_failure_fields_for_success() {
        let value = serde_json::to_value(build_report(&[passed("go/gopls")]))
            .expect("report should serialize");

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["cases"][0]["outcome"], "passed");
        assert!(value["cases"][0].get("failure_stage").is_none());
        assert!(value["cases"][0].get("message").is_none());
    }

    #[test]
    fn unwritable_result_path_has_user_facing_context() {
        let directory = tempfile::tempdir().expect("temporary directory should initialize");
        let path = directory.path().join("missing/results.json");

        let error = write_file(&path, b"results").expect_err("missing parent should fail");

        assert!(error.contains("failed to write E2E result file"));
        assert!(error.contains("results.tmp"));
    }
}
