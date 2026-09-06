use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fs_extra::dir::CopyOptions;

use super::E2eContext;

const LOG_EXCERPT_LIMIT: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SocketSnapshot {
    pub(crate) path: PathBuf,
    device: u64,
    inode: u64,
}

impl E2eContext {
    pub(crate) fn copy_project_as(&self, source: &Path, name: &str) -> Result<PathBuf, String> {
        let destination = self._sandbox.path().join(name);
        fs::create_dir(&destination).map_err(|error| {
            format!(
                "failed to create alternate E2E workspace {}: {error}",
                destination.display()
            )
        })?;
        fs_extra::dir::copy(source, &destination, &CopyOptions::new().content_only(true)).map_err(
            |error| {
                format!(
                    "failed to copy E2E project {} into {}: {error}",
                    source.display(),
                    destination.display()
                )
            },
        )?;
        Ok(destination)
    }

    pub(crate) fn daemon_sockets(&self) -> Result<Vec<SocketSnapshot>, String> {
        let root = self.runtime_dir.join("lsp-cli");
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(format!("failed to inspect {}: {error}", root.display()));
            }
        };
        let mut sockets = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let file_type = entry.file_type().ok()?;
                if !file_type.is_socket() {
                    return None;
                }
                let metadata = entry.metadata().ok()?;
                Some(SocketSnapshot {
                    path: entry.path(),
                    device: metadata.dev(),
                    inode: metadata.ino(),
                })
            })
            .collect::<Vec<_>>();
        sockets.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(sockets)
    }

    pub(crate) fn lifecycle_state(&self) -> String {
        let sockets = self.daemon_sockets().map_or_else(
            |error| error,
            |sockets| {
                if sockets.is_empty() {
                    "daemon sockets: none".to_string()
                } else {
                    format!("daemon sockets: {sockets:#?}")
                }
            },
        );
        let log_path = self.home.join(".local/share/lsp-cli/lsp-cli.log");
        let log = fs::read(&log_path).map_or_else(
            |error| format!("{} unavailable: {error}", log_path.display()),
            |bytes| {
                let start = bytes.len().saturating_sub(LOG_EXCERPT_LIMIT);
                String::from_utf8_lossy(bytes.get(start..).unwrap_or_default()).into_owned()
            },
        );
        format!("{sockets}\nsystem log:\n{log}")
    }

    pub(crate) fn server_start_count(&self) -> usize {
        self.server_pids().len()
    }

    pub(crate) fn server_pids(&self) -> Vec<u32> {
        let path = self.home.join(".local/share/lsp-cli/lsp-cli.log");
        fs::read_to_string(path)
            .map(|log| {
                log.lines()
                    .filter_map(|line| {
                        line.split("LSP server has started (pid ")
                            .nth(1)?
                            .strip_suffix(')')?
                            .parse()
                            .ok()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn wait_for_processes_to_exit(
        &self,
        pids: &[u32],
        timeout: Duration,
    ) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            let live = pids
                .iter()
                .copied()
                .filter(|pid| Path::new("/proc").join(pid.to_string()).exists())
                .collect::<Vec<_>>();
            if live.is_empty() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "server processes {live:?} remained alive after stop\n{}",
                    self.lifecycle_state()
                ));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
