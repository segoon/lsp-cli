use std::path::Path;

use super::E2eContext;
use crate::dependencies::ManagedDependencies;
use crate::results::{E2eFailure, E2eResult, FailureStage};

trait CleanupFailure: Sized {
    fn append(self, detail: String) -> Self;
    fn cleanup(detail: String) -> Self;
    fn add_cleanup(self, detail: String) -> Self;
}

impl CleanupFailure for String {
    fn append(self, detail: String) -> Self {
        format!("{self}\n{detail}")
    }

    fn cleanup(detail: String) -> Self {
        detail
    }

    fn add_cleanup(self, detail: String) -> Self {
        format!("{self}\n{detail}")
    }
}

impl CleanupFailure for E2eFailure {
    fn append(mut self, detail: String) -> Self {
        self = self.with_additional_detail(detail);
        self
    }

    fn cleanup(detail: String) -> Self {
        Self::new(FailureStage::Cleanup, detail)
    }

    fn add_cleanup(self, detail: String) -> Self {
        self.with_additional(FailureStage::Cleanup, detail)
    }
}

impl E2eContext {
    pub(super) fn isolated_roots(&self) -> [std::path::PathBuf; 2] {
        [
            self.home
                .parent()
                .expect("isolated home should have a sandbox parent")
                .to_path_buf(),
            self.runtime_dir.clone(),
        ]
    }

    pub(crate) fn run_cleaned(
        operation: impl FnOnce(&Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let context = Self::new()
            .map_err(|error| format!("failed to create an isolated E2E context: {error}"));
        Self::run_cleaned_context(context, operation)
    }

    pub(crate) fn run_cleaned_real_server(
        dependencies: &ManagedDependencies,
        operation: impl FnOnce(&Self) -> E2eResult,
    ) -> E2eResult {
        let context = Self::new_for_real_server(dependencies)
            .map_err(|error| E2eFailure::new(FailureStage::Setup, error));
        Self::run_cleaned_context(context, operation)
    }

    fn run_cleaned_context<E: CleanupFailure>(
        context: Result<Self, E>,
        operation: impl FnOnce(&Self) -> Result<(), E>,
    ) -> Result<(), E> {
        let context = context?;
        let [cache_root, runtime_root] = context.isolated_roots();
        let result = operation(&context);
        let retained = context.retained_failure_state();

        // Drop here, instead of at function exit, so every real-server case verifies that its
        // downloaded packages, compiler caches, temporary build output, and runtime state vanish.
        drop(context);
        let roots = [
            ("sandbox", cache_root.as_path()),
            ("runtime", runtime_root.as_path()),
        ];
        let cleanup_state = roots
            .iter()
            .map(|(label, path)| {
                let status = if path.exists() {
                    "not removed"
                } else {
                    "removed"
                };
                format!("{label} root: {status} ({})", path.display())
            })
            .collect::<Vec<_>>()
            .join("\n");
        let leftovers = roots
            .into_iter()
            .filter(|(_, path)| path.exists())
            .map(|(_, path)| Path::display(path).to_string())
            .collect::<Vec<_>>();
        let cleanup = leftovers.is_empty().then_some(()).ok_or_else(|| {
            format!(
                "isolated E2E download cache was not removed: {}",
                leftovers.join(", ")
            )
        });

        match (result, cleanup) {
            (Err(case), cleanup) => {
                let case = case.append(format!(
                    "retained E2E failure context:\n{retained}\ncleanup state:\n{cleanup_state}"
                ));
                if let Err(error) = cleanup {
                    Err(case.add_cleanup(error))
                } else {
                    Err(case)
                }
            }
            (Ok(()), Err(error)) => Err(E::cleanup(format!(
                "{error}\nretained E2E failure context:\n{retained}\ncleanup state:\n{cleanup_state}"
            ))),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_all_case_cache_roots_before_returning() {
        let mut roots = Vec::new();
        E2eContext::run_cleaned(|context| {
            roots.extend(context.isolated_roots());
            std::fs::write(context.home.join("download-cache"), b"cache")
                .expect("cache fixture should be created");
            Ok(())
        })
        .expect("case cleanup should succeed");

        assert!(roots.into_iter().all(|root| !root.exists()));
    }
}
