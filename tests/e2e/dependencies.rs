use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use sha2::{Sha256, Sha512};

#[path = "dependencies/catalog.rs"]
mod catalog;
use catalog::{Dependency, Digest};

use crate::repository_root;

#[path = "../../src/http/retry.rs"]
mod retry;
use retry::{Failure, RetryPolicy};

const AUTO_DOWNLOAD_ENV: &str = "E2E_AUTO_DOWNLOAD";

pub(crate) struct ManagedDependencies {
    install_path: OsString,
}

impl ManagedDependencies {
    pub(crate) fn prepare() -> Result<Self, String> {
        let host_path = std::env::var_os("PATH").unwrap_or_default();
        if !auto_download(std::env::var(AUTO_DOWNLOAD_ENV))? {
            return Ok(Self {
                install_path: host_path,
            });
        }

        require_program("tar")?;
        require_program("xz")?;
        let cache = Cache::new(repository_root().join(".env"))?;
        let catalog = catalog::dependencies()?;
        let installations = cache.ensure_all(&catalog)?;
        let install_path = assemble_install_path(&installations, &host_path)?;
        Ok(Self { install_path })
    }

    pub(crate) fn install_path(&self) -> &std::ffi::OsStr {
        &self.install_path
    }
}

fn auto_download(value: Result<String, std::env::VarError>) -> Result<bool, String> {
    match value.as_deref() {
        Ok("0") => Ok(false),
        Ok("1") | Err(std::env::VarError::NotPresent) => Ok(true),
        Ok(_) => Err(format!("{AUTO_DOWNLOAD_ENV} must be 0 or 1")),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("{AUTO_DOWNLOAD_ENV} must contain valid UTF-8"))
        }
    }
}

fn assemble_install_path(
    installations: &[(&Dependency, PathBuf)],
    host_path: &std::ffi::OsStr,
) -> Result<OsString, String> {
    let paths = installations
        .iter()
        .map(|(dependency, root)| root.join(dependency.path_directory))
        .chain(std::env::split_paths(host_path))
        .collect::<Vec<_>>();
    std::env::join_paths(paths)
        .map_err(|error| format!("failed to assemble E2E dependency PATH: {error}"))
}

struct Cache {
    root: PathBuf,
    client: Client,
}

impl Cache {
    fn new(root: PathBuf) -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .retry(reqwest::retry::never())
            .build()
            .map_err(|error| format!("failed to create E2E download client: {error}"))?;
        Ok(Self { root, client })
    }

    fn ensure_all<'a>(
        &self,
        dependencies: &'a [Dependency],
    ) -> Result<Vec<(&'a Dependency, PathBuf)>, String> {
        fs::create_dir_all(&self.root).map_err(|error| {
            format!(
                "failed to create E2E dependency cache {}: {error}",
                self.root.display()
            )
        })?;
        let _lock = CacheLock::acquire(&self.root.join("locks/setup"))?;
        dependencies
            .iter()
            .map(|dependency| self.ensure(dependency).map(|path| (dependency, path)))
            .collect()
    }

    fn ensure(&self, dependency: &Dependency) -> Result<PathBuf, String> {
        let installation = self.installation_path(dependency);
        if installation.exists() {
            return validate_installation(dependency, &installation).map(|()| installation).map_err(
                |error| {
                    format!(
                        "cached E2E {} installation is invalid: {error}; run 'make clean-e2e-dependencies' to remove cached installations",
                        dependency.name
                    )
                },
            );
        }

        let archive = self.ensure_download(dependency)?;
        let parent = installation.parent().ok_or_else(|| {
            format!(
                "failed to determine installation parent for {}",
                installation.display()
            )
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create E2E {} installation directory {}: {error}",
                dependency.name,
                parent.display()
            )
        })?;
        let staging = tempfile::Builder::new()
            .prefix(".installing-")
            .tempdir_in(parent)
            .map_err(|error| {
                format!(
                    "failed to create temporary E2E {} installation: {error}",
                    dependency.name
                )
            })?;
        extract(dependency, &archive, staging.path())?;
        if dependency.name == "ruby" {
            rewrite_ruby_shebangs(staging.path(), &installation)?;
        }
        fs::write(staging.path().join(".lsp-cli-version"), dependency.version).map_err(
            |error| {
                format!(
                    "failed to record E2E {} installation version: {error}",
                    dependency.name
                )
            },
        )?;
        validate_installation(dependency, staging.path())?;
        fs::rename(staging.path(), &installation).map_err(|error| {
            format!(
                "failed to publish E2E {} installation at {}: {error}",
                dependency.name,
                installation.display()
            )
        })?;
        Ok(installation)
    }

    fn ensure_download(&self, dependency: &Dependency) -> Result<PathBuf, String> {
        self.ensure_download_with(dependency, &RetryPolicy::default(), std::thread::sleep)
    }

    fn ensure_download_with(
        &self,
        dependency: &Dependency,
        policy: &RetryPolicy,
        sleep: impl Fn(Duration) + 'static,
    ) -> Result<PathBuf, String> {
        let destination = self
            .root
            .join("downloads")
            .join(dependency.digest.algorithm())
            .join(dependency.digest.expected())
            .join(dependency.archive_name);
        if destination.exists() {
            if verify_digest(&destination, dependency.digest)? {
                return Ok(destination);
            }
            fs::remove_file(&destination).map_err(|error| {
                format!(
                    "cached E2E {} download failed checksum validation and could not be removed: {error}",
                    dependency.name
                )
            })?;
        }
        let parent = destination.parent().expect("download path has a parent");
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create E2E download cache {}: {error}",
                parent.display()
            )
        })?;
        eprintln!("Downloading {} {}", dependency.name, dependency.version);
        policy
            .run(
                self.client.get(&dependency.url),
                |response| {
                    let mut temporary =
                        tempfile::NamedTempFile::new_in(parent).map_err(Failure::permanent)?;
                    DownloadResponse::new(response).copy_to(temporary.as_file_mut())?;
                    if !verify_digest(temporary.path(), dependency.digest)
                        .map_err(Failure::permanent)?
                    {
                        return Err(Failure::permanent(format!(
                            "download failed {} checksum validation",
                            dependency.digest.algorithm()
                        )));
                    }
                    temporary.persist(&destination).map_err(|error| {
                        Failure::permanent(format!("failed to cache download: {}", error.error))
                    })?;
                    Ok(destination.clone())
                },
                sleep,
            )
            .map_err(|error| {
                format!(
                    "failed to download E2E {} from {}: {error}",
                    dependency.name, dependency.url
                )
            })
    }

    fn installation_path(&self, dependency: &Dependency) -> PathBuf {
        self.root
            .join("installations")
            .join(dependency.name)
            .join(dependency.version)
            .join(&dependency.platform)
            .join(dependency.digest.expected())
    }
}

/// `io::copy` reports both read and write errors as `io::Error`. Track which side
/// failed so interrupted HTTP bodies retry, while local disk failures fail immediately.
struct DownloadResponse {
    response: Response,
    read_failed: bool,
}

impl DownloadResponse {
    fn new(response: Response) -> Self {
        Self {
            response,
            read_failed: false,
        }
    }

    fn copy_to(&mut self, writer: &mut impl std::io::Write) -> Result<u64, Failure> {
        std::io::copy(&mut *self, writer).map_err(|error| {
            if self.read_failed {
                Failure::network(error)
            } else {
                Failure::permanent(error)
            }
        })
    }
}

impl Read for DownloadResponse {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let result = self.response.read(buffer);
        self.read_failed |= result.is_err();
        result
    }
}

struct CacheLock {
    path: PathBuf,
}

impl CacheLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        let parent = path.parent().expect("lock path has a parent");
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create E2E dependency lock directory: {error}"))?;
        for _ in 0..600 {
            match fs::create_dir(path) {
                Ok(()) => {
                    fs::write(path.join("pid"), std::process::id().to_string()).map_err(
                        |error| format!("failed to record E2E dependency lock owner: {error}"),
                    )?;
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if lock_is_stale(path) {
                        remove_lock(path).map_err(|error| {
                            format!("failed to remove stale E2E dependency lock: {error}")
                        })?;
                    } else {
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
                Err(error) => {
                    return Err(format!("failed to acquire E2E dependency lock: {error}"));
                }
            }
        }
        Err("timed out waiting for another E2E dependency setup to finish".to_string())
    }
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        // Explicit cleanup is required so failed setup does not block every later test invocation.
        if let Err(error) = remove_lock(&self.path) {
            eprintln!("failed to remove E2E dependency lock: {error}");
        }
    }
}

fn remove_lock(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path.join("pid")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::remove_dir(path)
}

fn lock_is_stale(path: &Path) -> bool {
    fs::read_to_string(path.join("pid"))
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .is_some_and(|pid| !Path::new("/proc").join(pid.to_string()).exists())
}

fn extract(dependency: &Dependency, archive: &Path, destination: &Path) -> Result<(), String> {
    let mut command = Command::new("tar");
    command
        .args(["-xf"])
        .arg(archive)
        .arg("-C")
        .arg(destination);
    if dependency.strip_components > 0 {
        command.arg(format!(
            "--strip-components={}",
            dependency.strip_components
        ));
    }
    let output = command.output().map_err(|error| {
        format!(
            "failed to start tar while installing E2E {}: {error}",
            dependency.name
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "failed to extract E2E {}: {}",
            dependency.name,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn validate_installation(dependency: &Dependency, root: &Path) -> Result<(), String> {
    let validation = &dependency.validation;
    let program = root.join(validation.program);
    if !program.is_file() {
        return Err(format!("{} is missing", program.display()));
    }
    let mut command = Command::new(&program);
    command.args(validation.args);
    if dependency.name == "dotnet" {
        command.env("DOTNET_ROOT", root);
    } else if dependency.name == "ruby" {
        command.env("LD_LIBRARY_PATH", root.join("lib"));
    }
    let output = command.output().map_err(|error| {
        format!(
            "failed to run cached E2E {} executable {}: {error}",
            dependency.name,
            program.display()
        )
    })?;
    let actual = if validation.include_stderr {
        String::from_utf8_lossy(&output.stderr)
    } else {
        String::from_utf8_lossy(&output.stdout)
    };
    if output.status.success() && actual.contains(validation.expected) {
        Ok(())
    } else {
        Err(format!(
            "{} did not report expected version {:?}",
            program.display(),
            validation.expected
        ))
    }
}

fn verify_digest(path: &Path, digest: Digest) -> Result<bool, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("failed to read cached download {}: {error}", path.display()))?;
    let mut buffer = [0_u8; 64 * 1024];
    let actual = match digest {
        Digest::Sha256(_) => hash_reader::<Sha256>(&mut file, &mut buffer)?,
        Digest::Sha512(_) => hash_reader::<Sha512>(&mut file, &mut buffer)?,
    };
    Ok(actual == digest.expected())
}

fn hash_reader<D: sha2::Digest + Default>(
    reader: &mut impl Read,
    buffer: &mut [u8],
) -> Result<String, String> {
    let mut hasher = D::default();
    loop {
        let read = reader
            .read(buffer)
            .map_err(|error| format!("failed while hashing cached E2E download: {error}"))?;
        if read == 0 {
            break;
        }
        let bytes = buffer
            .get(..read)
            .ok_or_else(|| "failed to select bytes read for E2E download checksum".to_string())?;
        hasher.update(bytes);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(encoded, "{byte:02x}")
            .map_err(|error| format!("failed to format E2E download checksum: {error}"))?;
    }
    Ok(encoded)
}

fn rewrite_ruby_shebangs(staging: &Path, installation: &Path) -> Result<(), String> {
    let bin = staging.join("bin");
    for entry in fs::read_dir(&bin)
        .map_err(|error| format!("failed to read downloaded Ruby scripts: {error}"))?
    {
        let path = entry
            .map_err(|error| format!("failed to read a downloaded Ruby script: {error}"))?
            .path();
        let Ok(mut contents) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(line_end) = contents.find('\n') else {
            continue;
        };
        let Some(shebang) = contents.get(..line_end) else {
            continue;
        };
        if shebang.starts_with("#!") && shebang.ends_with("/bin/ruby") {
            contents.replace_range(
                ..line_end,
                &format!("#!{}", installation.join("bin/ruby").display()),
            );
            fs::write(&path, contents).map_err(|error| {
                format!(
                    "failed to update downloaded Ruby script {}: {error}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn require_program(name: &str) -> Result<(), String> {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|_| ())
        .map_err(|error| format!("E2E dependency setup requires {name:?} on PATH: {error}"))
}

#[cfg(test)]
#[path = "dependencies/tests.rs"]
mod tests;
