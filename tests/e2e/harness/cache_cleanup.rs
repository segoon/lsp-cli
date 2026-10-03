use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use super::E2eContext;
use crate::child_reaper::ChildReaper;
use crate::dependencies::ManagedDependencies;
use crate::results::{E2eFailure, E2eResult, FailureStage};

pub(crate) trait CleanupFailure: Sized {
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
    pub(crate) fn isolated_roots(&self) -> [std::path::PathBuf; 2] {
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
            .map_err(|error| E2eFailure::new(FailureStage::Setup, error))
            .and_then(|context| {
                let reaper = ChildReaper::begin_case()
                    .map_err(|error| E2eFailure::new(FailureStage::Setup, error))?;
                Ok(context.with_reaper(reaper))
            });
        Self::run_cleaned_context(context, operation)
    }

    pub(crate) fn with_reaper(mut self, reaper: ChildReaper) -> Self {
        // An abandoned context must retain files that a descendant could still be writing.
        self._sandbox.disable_cleanup(true);
        self._runtime_sandbox.disable_cleanup(true);
        self.reaper = Some(reaper);
        self
    }

    pub(crate) fn run_cleaned_context<E: CleanupFailure>(
        context: Result<Self, E>,
        operation: impl FnOnce(&Self) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut context = context?;
        let [cache_root, runtime_root] = context.isolated_roots();
        // We only catch unwinding to finalize resources, then resume the original panic. No
        // possibly inconsistent test state is reused after the panic.
        let outcome = catch_unwind(AssertUnwindSafe(|| operation(&context)));
        let retained = context.retained_failure_state();
        let cleanup = context.finalize();
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
        let result = match outcome {
            Ok(result) => result,
            Err(panic) => {
                if let Err(error) = cleanup {
                    eprintln!("E2E cleanup failed while unwinding: {error}\n{cleanup_state}");
                }
                resume_unwind(panic);
            }
        };

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

    fn finalize(&mut self) -> Result<(), String> {
        self.finalized = true;
        self._sandbox.disable_cleanup(true);
        self._runtime_sandbox.disable_cleanup(true);
        let mut errors = Vec::new();
        if let Err(error) = self.stop_daemons() {
            errors.push(error);
        }
        let descendants = match &mut self.reaper {
            Some(reaper) => reaper.finish(),
            None => Ok(()),
        };
        match descendants {
            Err(error) => errors.push(error),
            Ok(()) => {
                for root in self.isolated_roots() {
                    if let Err(error) = std::fs::remove_dir_all(&root)
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        errors.push(format!(
                            "failed to remove E2E directory {}: {error}",
                            root.display()
                        ));
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
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
