use std::path::Path;

use super::{ExceptionOutcome, LspConfig, ProvisionMethod, QueryKind, RealServerCase, read_yaml};

impl RealServerCase<'_> {
    pub(crate) fn label(&self) -> String {
        format!("{}/{}", self.pair.language, self.pair.server)
    }

    pub(crate) fn language(&self) -> &str {
        &self.language.id
    }

    pub(crate) fn server_name(&self, repository: &Path) -> Result<String, String> {
        server_name(&self.pair.server, repository)
    }

    pub(crate) fn project(&self) -> &Path {
        &self.language.project
    }

    pub(crate) fn host_programs(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.setup
            .host_programs
            .iter()
            .map(|program| (program.name.as_str(), program.resolve.as_slice()))
    }

    pub(crate) fn provision_method(&self) -> ProvisionMethod {
        self.setup.provision.method
    }

    pub(crate) fn symbol_query(&self) -> &str {
        self.symbol_query
    }

    pub(crate) fn callable_query(&self) -> &str {
        self.callable_query
    }

    pub(crate) fn format_file(&self) -> &Path {
        self.format_file
    }

    pub(crate) fn expected_names(&self) -> &[String] {
        self.expected_names
    }

    pub(crate) fn exception(
        &self,
        command: QueryKind,
    ) -> Option<(ExceptionOutcome, Option<&str>, &str)> {
        self.exceptions
            .iter()
            .find(|item| item.command == command)
            .map(|item| (item.outcome, item.message.as_deref(), item.reason.as_str()))
    }

    pub(crate) fn lsp_timeout_seconds(&self) -> u64 {
        self.lsp_timeout_seconds
    }

    pub(crate) fn deadline_seconds(&self) -> u64 {
        self.deadline_seconds
    }
}

pub(super) fn server_name(server: &str, repository: &Path) -> Result<String, String> {
    let path = repository.join("data/lsp").join(format!("{server}.yaml"));
    let config: LspConfig = read_yaml(&path)?;
    Ok(config.name)
}
