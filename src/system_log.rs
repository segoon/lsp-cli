use crate::runtime_state::{RuntimeState, default_runtime_state_root};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, ExitStatus};

const MAX_LOG_FILE_SIZE: u64 = 10 * 1024 * 1024;

pub(crate) fn warn_if_log_file_is_large() {
    let Some(log_path) = default_log_path() else {
        return;
    };
    let Ok(metadata) = std::fs::metadata(&log_path) else {
        return;
    };
    let Some(message) = log_size_warning_message(&log_path, metadata.len()) else {
        return;
    };
    eprintln!("{message}");
}

pub(crate) fn log_lsp_server_starting() {
    append_system_log_line("starting LSP server...");
}

pub(crate) fn log_lsp_server_started(server_pid: u32) {
    append_system_log_line(&format!("LSP server has started (pid {server_pid})"));
}

pub(crate) fn log_lsp_server_exit(status: ExitStatus) {
    append_system_log_line(&format!(
        "LSP server exited with {}",
        format_exit_status(status)
    ));
}

pub(crate) fn log_lsp_server_stderr_line(line: &str) {
    append_system_log_line(&format!("stderr: {line}"));
}

pub(crate) fn log_unexpected_error(error: &str) {
    append_system_log_line(&format!("unexpected error: {error}"));
}

pub(crate) fn append_system_log_line(message: &str) {
    let Some(log_path) = default_log_path() else {
        return;
    };
    let Some(parent) = log_path.parent() else {
        return;
    };

    if std::fs::create_dir_all(parent).is_err() {
        return;
    }

    let Ok(mut file) = LockedLogFile::open(&log_path) else {
        return;
    };

    for line in message.lines() {
        if writeln!(file, "{}: {line}", process::id()).is_err() {
            break;
        }
    }
}

struct LockedLogFile {
    file: std::fs::File,
}

impl LockedLogFile {
    fn open(path: &Path) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        file.lock()?;
        Ok(Self { file })
    }
}

impl Write for LockedLogFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.file.flush()
    }
}

impl Drop for LockedLogFile {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
pub(crate) fn append_system_log_line_to_file(file: &mut std::fs::File, pid: u32, message: &str) {
    for line in message.lines() {
        writeln!(file, "{pid}: {line}").expect("log line should write");
    }
}

pub(crate) fn log_size_warning_message(log_path: &Path, size: u64) -> Option<String> {
    (size > MAX_LOG_FILE_SIZE).then(|| {
        format!(
            "warning: global log file {} is larger than 10 MiB",
            log_path.display()
        )
    })
}

pub(crate) fn format_exit_status(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;

        if let Some(signal) = status.signal() {
            return format!("signal {signal}");
        }
    }

    status.code().map_or_else(
        || "unknown status".to_string(),
        |code| format!("exit code {code}"),
    )
}

fn default_log_path() -> Option<PathBuf> {
    default_runtime_state_root()
        .ok()
        .map(RuntimeState::new)
        .map(|state| state.log_path())
}

#[cfg(test)]
mod tests {
    use super::{append_system_log_line_to_file, format_exit_status, log_size_warning_message};
    use crate::test_support::TestDir;
    use std::fs::{self, File};
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    #[test]
    fn prefixes_each_log_line_with_pid() {
        let dir = TestDir::new("system-log");
        let path = dir.path().join("system.log");
        let mut file = File::create(&path).expect("log file should be created");

        append_system_log_line_to_file(&mut file, 1345, "first line\nsecond line");

        assert_eq!(
            fs::read_to_string(path).expect("log file should be readable"),
            "1345: first line\n1345: second line\n"
        );
    }

    #[test]
    fn warns_when_log_file_is_larger_than_limit() {
        let message =
            log_size_warning_message(std::path::Path::new("/tmp/lsp-cli.log"), 10_485_761)
                .expect("large log file should warn");

        assert!(message.contains("/tmp/lsp-cli.log"));
        assert!(message.contains("10 MiB"));
    }

    #[test]
    fn does_not_warn_when_log_file_is_small_enough() {
        assert!(
            log_size_warning_message(std::path::Path::new("/tmp/lsp-cli.log"), 10_485_760)
                .is_none()
        );
    }

    #[test]
    fn formats_exit_code_status() {
        let status = ExitStatus::from_raw(3 << 8);

        assert_eq!(format_exit_status(status), "exit code 3");
    }

    #[test]
    fn formats_signal_status() {
        let status = ExitStatus::from_raw(15);

        assert_eq!(format_exit_status(status), "signal 15");
    }
}
