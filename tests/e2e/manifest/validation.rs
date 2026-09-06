use std::path::{Component, Path};

use super::LanguageCase;

impl LanguageCase {
    pub(super) fn validate_project(&self, repository: &Path) -> Result<(), String> {
        if self.project.is_absolute()
            || self
                .project
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!(
                "E2E project path {} must be a normalized relative path",
                self.project.display()
            ));
        }

        let project = repository.join(&self.project);
        if !project.is_dir() {
            return Err(format!(
                "E2E {} project {} for language {:?} is not a directory",
                self.kind.label(),
                self.project.display(),
                self.id
            ));
        }
        let canonical_repository = repository
            .canonicalize()
            .map_err(|error| format!("failed to resolve {}: {error}", repository.display()))?;
        let canonical_project = project
            .canonicalize()
            .map_err(|error| format!("failed to resolve {}: {error}", project.display()))?;
        if !canonical_project.starts_with(canonical_repository) {
            return Err(format!(
                "E2E project {} resolves outside the repository",
                self.project.display()
            ));
        }
        Ok(())
    }
}

pub(super) fn validate_config_id(kind: &str, value: &str) -> Result<(), String> {
    let mut components = Path::new(value).components();
    if value.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(format!(
            "E2E {kind} config ID {value:?} must be one normalized path component"
        ));
    }
    Ok(())
}
