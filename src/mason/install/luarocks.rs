use super::{
    installer_command, prepare_install_dir, require_command, run_install_command,
    use_cached_program_or,
};
use crate::error::Result;
use crate::mason::link::finalize_install;
use crate::mason::registry::MasonPackage;
use crate::mason::source::LuaRocksSource;
use crate::mason::template::TemplateContext;
use crate::runtime_state::RuntimeState;
use std::path::{Path, PathBuf};
use std::process::Command;

impl LuaRocksSource {
    pub(super) fn install(
        &self,
        state: &RuntimeState,
        package: &MasonPackage,
        program: &str,
    ) -> Result<PathBuf> {
        use_cached_program_or(
            package,
            program,
            state,
            &TemplateContext::empty(),
            |resolved| {
                require_command("luarocks", package, program)?;
                let install_dir = prepare_install_dir(state, package)?;
                // LuaRocks embeds absolute paths in launchers; install into the final tree,
                // rather than moving a temporary installation after it succeeds.
                let mut command = self.install_command(&install_dir);
                run_install_command(&mut command, package, "luarocks")?;
                finalize_install(
                    state,
                    package,
                    program,
                    &resolved,
                    &TemplateContext::empty(),
                    "LuaRocks did not produce a runnable",
                )
            },
        )
    }

    fn install_command(&self, install_dir: &Path) -> Command {
        let mut command = installer_command("luarocks");
        command.arg("install").arg("--tree").arg(install_dir);
        if self.dev {
            command.arg("--dev");
        }
        if let Some(server) = &self.server {
            command.arg(format!("--server={server}"));
        }
        command.arg(&self.package_name).arg(&self.version);
        command
    }
}

#[cfg(test)]
mod tests;
