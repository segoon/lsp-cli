use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::E2eContext;

const CAPABILITIES_FILE: &str = "retained-server-capabilities.json";
const DIAGNOSTIC_EXCERPT_LIMIT: usize = 16 * 1024;

#[derive(Deserialize)]
struct InstallReceipt {
    source_id: String,
}

impl E2eContext {
    pub(crate) fn record_server_capabilities(&self, capabilities: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(capabilities)
            .map_err(|error| format!("failed to serialize server capabilities: {error}"))?;
        fs::write(self._sandbox.path().join(CAPABILITIES_FILE), bytes)
            .map_err(|error| format!("failed to retain server capabilities: {error}"))
    }

    pub(super) fn retained_failure_state(&self) -> String {
        let system_log = self.system_log();
        format!(
            "server package source IDs:\n{}\nserver command line:\n{}\nserver capabilities:\n{}\nserver stderr summary:\n{}",
            self.package_source_ids(),
            latest_log_value(&system_log, "LSP server cmdline: "),
            read_excerpt(&self._sandbox.path().join(CAPABILITIES_FILE)),
            stderr_summary(&system_log),
        )
    }

    fn package_source_ids(&self) -> String {
        let directory = self.home.join(".local/share/lsp-cli/receipts");
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return "<unavailable: no completed server installation>".to_string();
            }
            Err(error) => return format!("<unavailable: {error}>"),
        };
        let mut sources = entries
            .filter_map(Result::ok)
            .filter_map(|entry| fs::read(entry.path()).ok())
            .filter_map(|bytes| serde_json::from_slice::<InstallReceipt>(&bytes).ok())
            .map(|receipt| receipt.source_id)
            .collect::<Vec<_>>();
        sources.sort();
        sources.dedup();
        if sources.is_empty() {
            "<unavailable: no readable server installation receipt>".to_string()
        } else {
            sources.join("\n")
        }
    }

    fn system_log(&self) -> String {
        let path = self.home.join(".local/share/lsp-cli/lsp-cli.log");
        read_excerpt(&path)
    }
}

fn read_excerpt(path: &Path) -> String {
    match fs::read(path) {
        Ok(bytes) => {
            let truncated = bytes.len() > DIAGNOSTIC_EXCERPT_LIMIT;
            let start = bytes.len().saturating_sub(DIAGNOSTIC_EXCERPT_LIMIT);
            let text = String::from_utf8_lossy(bytes.get(start..).unwrap_or_default());
            if truncated {
                format!("<earlier content truncated>\n{text}")
            } else {
                text.into_owned()
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => "<unavailable>".to_string(),
        Err(error) => format!("<unavailable: {error}>"),
    }
}

fn latest_log_value(log: &str, marker: &str) -> String {
    log.lines()
        .filter_map(|line| line.split_once(marker).map(|(_, value)| value))
        .next_back()
        .unwrap_or("<unavailable>")
        .to_string()
}

fn stderr_summary(log: &str) -> String {
    let lines = log
        .lines()
        .filter_map(|line| line.split_once(" stderr: ").map(|(_, value)| value))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "<none captured>".to_string()
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_case_retains_server_details_after_cleanup() {
        let mut roots = Vec::new();
        let diagnostic = E2eContext::run_cleaned(|context| {
            roots.extend(context.isolated_roots());
            let state = context.home.join(".local/share/lsp-cli");
            fs::create_dir_all(state.join("receipts"))
                .expect("receipt directory should be created");
            fs::write(
                state.join("receipts/server.json"),
                br#"{"source_id":"pkg:github/example/server@1.2.3"}"#,
            )
            .expect("receipt should be written");
            fs::write(
                state.join("lsp-cli.log"),
                "timestamp LSP server cmdline: /server --stdio\ntimestamp stderr: server-stderr-marker\n",
            )
            .expect("system log should be written");
            context
                .record_server_capabilities(&serde_json::json!({"hoverProvider": true}))
                .expect("capabilities should be retained");
            Err("synthetic case failure".to_string())
        })
        .expect_err("synthetic case should fail");

        for expected in [
            "synthetic case failure",
            "pkg:github/example/server@1.2.3",
            "/server --stdio",
            "hoverProvider",
            "server-stderr-marker",
            "sandbox root: removed",
            "runtime root: removed",
        ] {
            assert!(diagnostic.contains(expected), "missing {expected:?}");
        }
        assert!(roots.into_iter().all(|root| !root.exists()));
    }

    #[test]
    fn early_failure_labels_details_that_do_not_exist() {
        let diagnostic = E2eContext::run_cleaned(|_context| Err("early failure".to_string()))
            .expect_err("synthetic case should fail");

        assert!(diagnostic.contains("no completed server installation"));
        assert!(diagnostic.contains("server command line:\n<unavailable>"));
        assert!(diagnostic.contains("server capabilities:\n<unavailable>"));
        assert!(diagnostic.contains("server stderr summary:\n<none captured>"));
    }
}
