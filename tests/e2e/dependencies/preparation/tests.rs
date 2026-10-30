use super::*;
use crate::dependencies::catalog::{Digest, Validation};
use crate::dependencies::paths::PathEntry;
use std::os::unix::fs::PermissionsExt as _;

struct Fixture {
    directory: tempfile::TempDir,
    dependency: Dependency,
}

impl Fixture {
    fn new(preparation: Preparation) -> Self {
        Self {
            directory: tempfile::tempdir().expect("runtime fixture should initialize"),
            dependency: Dependency {
                name: "luarocks",
                version: "3.13.0",
                platform: "test-platform".to_string(),
                url: String::new(),
                archive_name: "fixture",
                digest: Digest::Sha256("fixture-digest"),
                strip_components: 0,
                preparation,
                path_entry: PathEntry::Directory("bin"),
                validation: Validation {
                    program: "bin/luarocks",
                    args: &["--version"],
                    expected: "3.13.0",
                    include_stderr: false,
                },
            },
        }
    }

    fn root(&self) -> std::path::PathBuf {
        self.directory.path().join("cache with spaces")
    }

    fn installation(&self, root: &Path) -> std::path::PathBuf {
        root.join("installations/luarocks")
            .join(self.dependency.cache_version())
            .join("test-platform/fixture-digest")
    }

    fn prepare_lua(&self) {
        let lua = self
            .root()
            .join("installations/lua")
            .join(Preparation::Lua.cache_version("5.4.9"))
            .join("test-platform/lua-digest/bin/lua");
        fs::create_dir_all(lua.parent().expect("interpreter has a parent"))
            .expect("interpreter directory should exist");
        // Print the invocation so relocation and interpreter selection are tested without a host Lua installation.
        fs::write(&lua, "#!/bin/sh\nprintf '%s\\n' \"$0\" \"$@\"\n")
            .expect("interpreter fixture should be written");
        make_executable(&lua).expect("interpreter should be executable");
    }
}

#[test]
fn standalone_executable_is_copied_with_safe_runnable_permissions() {
    let fixture = Fixture::new(Preparation::Executable);
    let source = fixture.directory.path().join("download");
    fs::write(&source, "#!/bin/sh\necho 3.13.0\n").expect("download should exist");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o7777))
        .expect("source mode should be set");
    let destination = fixture.installation(&fixture.root());
    fixture
        .dependency
        .extract(&source, &destination)
        .expect("executable should install");
    let program = destination.join("bin/luarocks");
    assert_eq!(
        fs::metadata(&program)
            .expect("program should exist")
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
    let output = Command::new(program).output().expect("program should run");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"3.13.0\n");
}

#[test]
fn luarocks_launcher_survives_cache_relocation_and_selects_managed_lua() {
    let fixture = Fixture::new(Preparation::LuaRocks {
        lua_version: "5.4.9",
        lua_digest: "lua-digest",
    });
    fixture.prepare_lua();
    let original = fixture.installation(&fixture.root());
    fixture
        .dependency
        .prepare(&original)
        .expect("launcher should be prepared");
    let restored = fixture.directory.path().join("restored cache with spaces");
    fs::rename(fixture.root(), &restored).expect("cache should be movable");
    let output = Command::new(fixture.installation(&restored).join("bin/luarocks"))
        .args([
            "install",
            "--tree",
            "server tree with spaces",
            "fennel-ls",
            "0.2.1-1",
        ])
        .output()
        .expect("restored launcher should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let args = String::from_utf8(output.stdout).expect("arguments should be UTF-8");
    assert!(args.contains(restored.to_str().expect("fixture path is UTF-8")));
    assert!(!args.contains(fixture.root().to_str().expect("fixture path is UTF-8")));
    assert!(args.contains(
        "--lua-version=5.4\ninstall\n--tree\nserver tree with spaces\nfennel-ls\n0.2.1-1\n"
    ));
}

#[test]
fn source_runtime_cache_identity_includes_its_interpreter_dependency() {
    let first = Fixture::new(Preparation::LuaRocks {
        lua_version: "5.4.9",
        lua_digest: "first",
    });
    for preparation in [
        Preparation::LuaRocks {
            lua_version: "5.4.8",
            lua_digest: "first",
        },
        Preparation::LuaRocks {
            lua_version: "5.4.9",
            lua_digest: "second",
        },
    ] {
        let other = Fixture::new(preparation);
        assert_ne!(
            first.dependency.cache_version(),
            other.dependency.cache_version()
        );
    }
    let archive = Fixture::new(Preparation::Archive);
    assert_eq!(archive.dependency.cache_version(), "3.13.0");
}

#[test]
fn lua_validation_rejects_missing_development_files() {
    let fixture = Fixture::new(Preparation::Lua);
    let root = fixture.root();
    for (directory, files) in LUA_DEVELOPMENT_FILES {
        fs::create_dir_all(root.join(directory)).expect("development directory should exist");
        for file in *files {
            fs::write(root.join(directory).join(file), "fixture")
                .expect("development file should exist");
        }
    }
    fixture
        .dependency
        .validate_support_files(&root)
        .expect("complete Lua development files should validate");
    for missing in ["include/lua.h", "lib/liblua.a"] {
        let path = root.join(missing);
        fs::remove_file(&path).expect("required file should be removable");
        let error = fixture
            .dependency
            .validate_support_files(&root)
            .expect_err("incomplete Lua must not be accepted from cache");
        assert!(error.contains(missing), "{error}");
        fs::write(path, "fixture").expect("required file should be restored");
    }
}
