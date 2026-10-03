use std::path::PathBuf;

use crate::harness::E2eContext;
use crate::manifest::{CommandStrategy, Manifest};

pub(crate) struct E2eFixture {
    context: E2eContext,
    manifest: Manifest,
}

impl E2eFixture {
    pub(crate) fn new() -> Result<Self, String> {
        Self::initialize(None)
    }

    pub(crate) fn new_with_data_dir(data_dir: PathBuf) -> Result<Self, String> {
        Self::initialize(Some(data_dir))
    }

    pub(crate) fn context(&self) -> &E2eContext {
        &self.context
    }

    pub(crate) fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub(crate) fn commands_for(&self, strategy: CommandStrategy) -> impl Iterator<Item = &str> {
        self.manifest.commands_for(strategy)
    }

    fn initialize(data_dir: Option<PathBuf>) -> Result<Self, String> {
        let manifest = Manifest::load_repository()?;
        let mut context =
            E2eContext::new().map_err(|error| format!("failed to create E2E context: {error}"))?;
        if let Some(data_dir) = data_dir {
            context = context.with_data_dir(data_dir);
        }
        Ok(Self { context, manifest })
    }
}
