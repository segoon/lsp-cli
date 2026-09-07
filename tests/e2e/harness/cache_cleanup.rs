use std::path::Path;

use super::E2eContext;

impl E2eContext {
    pub(crate) fn run_cleaned(
        operation: impl FnOnce(&Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let context = Self::new()
            .map_err(|error| format!("failed to create an isolated E2E context: {error}"))?;
        let cache_root = context
            .home
            .parent()
            .expect("isolated home should have a sandbox parent")
            .to_path_buf();
        let runtime_root = context.runtime_dir.clone();
        let result = operation(&context);

        // Drop here, instead of at function exit, so every real-server case verifies that its
        // downloaded packages, compiler caches, temporary build output, and runtime state vanish.
        drop(context);
        let leftovers = [cache_root.as_path(), runtime_root.as_path()]
            .into_iter()
            .filter(|path| path.exists())
            .map(Path::display)
            .map(|path| path.to_string())
            .collect::<Vec<_>>();
        let cleanup = leftovers.is_empty().then_some(()).ok_or_else(|| {
            format!(
                "isolated E2E download cache was not removed: {}",
                leftovers.join(", ")
            )
        });

        match (result, cleanup) {
            (Err(case), Err(cleanup)) => Err(format!("{case}\n{cleanup}")),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
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
            roots.push(
                context
                    .home
                    .parent()
                    .expect("home should have a parent")
                    .to_path_buf(),
            );
            roots.push(context.runtime_dir.clone());
            std::fs::write(context.home.join("download-cache"), b"cache")
                .expect("cache fixture should be created");
            Ok(())
        })
        .expect("case cleanup should succeed");

        assert!(roots.into_iter().all(|root| !root.exists()));
    }
}
