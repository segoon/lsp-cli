use std::fs;
use std::path::Path;
use std::process::Command;

use super::catalog::Dependency;

pub(super) enum Preparation {
    Archive,
    Executable,
    Lua,
    LuaRocks {
        lua_version: &'static str,
        lua_digest: &'static str,
    },
}

impl Preparation {
    fn cache_version(&self, version: &str) -> String {
        match self {
            Self::Archive => version.to_string(),
            Self::Executable => format!("{version}-executable-v1"),
            Self::Lua => format!("{version}-source-v1"),
            Self::LuaRocks {
                lua_version,
                lua_digest,
            } => format!("{version}-lua{lua_version}-{lua_digest}-source-v1"),
        }
    }
}

const LUA_DEVELOPMENT_FILES: &[(&str, &[&str])] = &[
    (
        "include",
        &["lua.h", "luaconf.h", "lualib.h", "lauxlib.h", "lua.hpp"],
    ),
    ("lib", &["liblua.a"]),
];

impl Dependency {
    pub(super) fn validate_support_files(&self, root: &Path) -> Result<(), String> {
        if matches!(self.preparation, Preparation::Lua) {
            for (directory, files) in LUA_DEVELOPMENT_FILES {
                for file in *files {
                    let path = root.join(directory).join(file);
                    if !path.is_file() {
                        return Err(format!("{} is missing", path.display()));
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn cache_version(&self) -> String {
        self.preparation.cache_version(self.version)
    }

    pub(super) fn extract(&self, archive: &Path, destination: &Path) -> Result<(), String> {
        if matches!(self.preparation, Preparation::Executable) {
            let target = destination.join(self.validation.program);
            let parent = target.parent().ok_or("E2E executable path has no parent")?;
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create E2E executable directory: {error}"))?;
            fs::copy(archive, &target).map_err(|error| {
                format!("failed to install E2E {} executable: {error}", self.name)
            })?;
            return make_executable(&target);
        }
        let mut command = Command::new("tar");
        command.arg("-xf").arg(archive).arg("-C").arg(destination);
        if self.strip_components > 0 {
            command.arg(format!("--strip-components={}", self.strip_components));
        }
        self.run(&mut command, "extract")
    }

    pub(super) fn prepare(&self, destination: &Path) -> Result<(), String> {
        match self.preparation {
            Preparation::Archive | Preparation::Executable => Ok(()),
            Preparation::Lua => self.build_lua(destination),
            Preparation::LuaRocks {
                lua_version,
                lua_digest,
            } => self.prepare_luarocks(destination, lua_version, lua_digest),
        }
    }

    fn run(&self, command: &mut Command, action: &str) -> Result<(), String> {
        let output = command
            .output()
            .map_err(|error| format!("failed to {action} E2E {}: {error}", self.name))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "failed to {action} E2E {}: {}",
                self.name,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    fn build_lua(&self, root: &Path) -> Result<(), String> {
        let mut command = Command::new("make");
        command
            .current_dir(root)
            .args(["linux", "MYCFLAGS=-fPIC", "-j2"])
            .env_remove("MAKEFLAGS")
            .env_remove("MFLAGS");
        self.run(&mut command, "build")?;
        // Upstream's `make install` expands its prefix into unquoted shell commands.
        // Copy the standard runtime/development files directly to support cache paths with spaces.
        for (directory, files) in std::iter::once(("bin", &["lua", "luac"][..]))
            .chain(LUA_DEVELOPMENT_FILES.iter().copied())
        {
            fs::create_dir_all(root.join(directory))
                .map_err(|error| format!("failed to create Lua {directory} directory: {error}"))?;
            for file in files {
                fs::copy(root.join("src").join(file), root.join(directory).join(file))
                    .map_err(|error| format!("failed to install Lua {file}: {error}"))?;
            }
        }
        Ok(())
    }

    fn prepare_luarocks(
        &self,
        root: &Path,
        lua_version: &str,
        lua_digest: &str,
    ) -> Result<(), String> {
        let lua_key = Preparation::Lua.cache_version(lua_version);
        // Both staging and published runtime directories occupy the same level in
        // installations/name/version/platform/digest, so this remains valid after cache restores.
        let lua = Path::new("../../../..")
            .join("lua")
            .join(lua_key)
            .join(&self.platform)
            .join(lua_digest);
        let script = format!(
            r#"#!/bin/sh
runtime_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd) || exit
lua_root=$(CDPATH= cd -- "$runtime_root/{lua}" && pwd) || exit
exec "$lua_root/bin/lua" -e '{bootstrap}' "$runtime_root/src/bin/luarocks" --lua-dir="$lua_root" --lua-version=5.4 "$@"
"#,
            lua = lua.display(),
            bootstrap = LUAROCKS_BOOTSTRAP
        );
        fs::create_dir_all(root.join("bin"))
            .map_err(|error| format!("failed to create LuaRocks launcher directory: {error}"))?;
        let path = root.join("bin/luarocks");
        fs::write(&path, script)
            .map_err(|error| format!("failed to write LuaRocks launcher: {error}"))?;
        make_executable(&path)
    }
}

// LuaRocks supports running from its source tree. Force only the managed configuration
// while deriving module paths at runtime; generated installed scripts embed absolute prefixes.
const LUAROCKS_BOOTSTRAP: &str = "local root=arg[0]:match(\"^(.*)/src/bin/luarocks$\"); package.path=root..\"/src/?.lua;\"..root..\"/src/?/init.lua;\"..package.path; package.loaded[\"luarocks.core.hardcoded\"]={FORCE_CONFIG=true,SYSCONFDIR=root..\"/etc/luarocks\"}";

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("failed to make {} executable: {error}", path.display()))
    }
    #[cfg(not(unix))]
    Err("managed E2E executable preparation requires Unix".to_string())
}

#[cfg(test)]
#[path = "preparation/tests.rs"]
mod tests;
