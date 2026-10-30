use super::{installer_command, prepare_install_dir, require_command, run_install_command};
use crate::error::{Error, Result};
use crate::mason::link::{finalize_install, is_resolved_program_runnable, resolve_program};
use crate::mason::registry::MasonPackage;
use crate::mason::source::CargoSource;
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use std::path::{Path, PathBuf};
use std::process::Command;

const CACHE_MARKER: &str = ".lsp-cli-cargo";

impl CargoSource {
    pub(super) fn resolve_cached_program(
        &self,
        state: &RuntimeState,
        package: &MasonPackage,
        program: &str,
    ) -> Result<Option<PathBuf>> {
        let marker = state.package_dir(&package.name).join(CACHE_MARKER);
        if !std::fs::read_to_string(marker)
            .is_ok_and(|value| value == self.cache_signature(package))
        {
            return Ok(None);
        }
        let resolved = resolve_program(package, program, state, &TemplateContext::empty())?;
        Ok(is_resolved_program_runnable(&resolved)
            .then(|| resolved.executable_path().to_path_buf()))
    }

    pub(super) fn install(
        &self,
        state: &RuntimeState,
        package: &MasonPackage,
        program: &str,
    ) -> Result<PathBuf> {
        if let Some(cached) = self.resolve_cached_program(state, package, program)? {
            return Ok(cached);
        }
        require_command("cargo", package, program)?;
        let install_dir = prepare_install_dir(state, package)?;
        // Invalidate the marker before running Cargo: a failed rebuild must never make
        // partially replaced binaries eligible for reuse, even if the user switches back.
        remove_if_present(&install_dir.join(CACHE_MARKER))?;
        let resolved = resolve_program(package, program, state, &TemplateContext::empty())?;
        // The signature is shared by every binary in this package. Clear Cargo's bin
        // directory so a formerly enabled executable cannot survive under the new signature.
        let bin_dir = install_dir.join("bin");
        if bin_dir.exists() {
            std::fs::remove_dir_all(&bin_dir).map_err(|error| {
                Error::unexpected(format!(
                    "failed to clear stale Cargo executables in {}: {error}",
                    bin_dir.display()
                ))
            })?;
        }
        run_install_command(&mut self.install_command(&install_dir), package, "cargo")?;
        let installed = finalize_install(
            state,
            package,
            program,
            &resolved,
            &TemplateContext::empty(),
            "cargo did not produce a runnable",
        )?;
        crate::fs::write(
            &install_dir.join(CACHE_MARKER),
            self.cache_signature(package).as_bytes(),
        )?;
        Ok(installed)
    }

    fn cache_signature(&self, package: &MasonPackage) -> String {
        // This private format includes a behavior revision so installations made before
        // Cargo qualifier/locking support are rebuilt once, without touching other packages.
        format!("cargo-v1\n{:?}\n{self:?}\n", package.source.id)
    }

    fn install_command(&self, directory: &Path) -> Command {
        let mut command = installer_command("cargo");
        command.arg("install").arg("--root").arg(directory);
        if let Some(repository) = &self.repository_url {
            command
                .arg("--git")
                .arg(repository)
                .arg(if self.rev { "--rev" } else { "--tag" })
                .arg(&self.version);
        } else {
            command.arg("--version").arg(&self.version);
        }
        if let Some(features) = &self.features {
            command.arg("--features").arg(features);
        }
        if self.locked {
            command.arg("--locked");
        }
        // A cache miss requires an actual build, even when Cargo's installation metadata
        // still describes the same crate/version (for example after changing locking).
        command.arg("--force").arg(&self.package_name);
        command
    }
}

fn remove_if_present(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::unexpected(format!(
            "failed to invalidate Cargo cache file {}: {error}",
            path.display()
        ))),
    }
}

#[cfg(test)]
mod tests;
