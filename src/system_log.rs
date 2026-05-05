use crate::runtime_state::{RuntimeState, default_runtime_state_root};
use humantime::format_rfc3339_millis;
use shlex::try_join as shell_try_join;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, ExitStatus};
use std::time::SystemTime;

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

pub(crate) fn log_lsp_server_cmdline(command: &[String]) {
    append_system_log_line(&format!("LSP server cmdline: {}", format_command(command)));
}

pub(crate) fn log_lsp_server_cwd(cwd: &Path) {
    append_system_log_line(&format!("LSP server cwd: {}", cwd.display()));
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
    let timestamp = current_log_timestamp();
    let pid = process::id();

    for line in message.lines() {
        if writeln!(file, "{timestamp} pid={pid} {line}").is_err() {
            break;
        }
    }
}

fn current_log_timestamp() -> String {
    format_rfc3339_millis(SystemTime::now()).to_string()
}

fn format_command(command: &[String]) -> String {
    shell_try_join(command.iter().map(String::as_str)).unwrap_or_else(|_| command.join(" "))
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
pub(crate) fn append_system_log_line_to_file(
    file: &mut std::fs::File,
    timestamp: &str,
    pid: u32,
    message: &str,
) {
    for line in message.lines() {
        writeln!(file, "{timestamp} pid={pid} {line}").expect("log line should write");
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
    use super::{
        append_system_log_line_to_file, current_log_timestamp, format_command, format_exit_status,
        log_size_warning_message,
    };
    use crate::test_support::TestDir;
    use shlex::split as shell_split;
    use std::fs::{self, File};
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    // Formatting-only fixture path used by tests below; they do not access this path.
    const SAMPLE_LOG_PATH: &str = "/tmp/lsp-cli.log";

    #[test]
    fn prefixes_each_log_line_with_timestamp_and_pid() {
        let dir = TestDir::new("system-log");
        let path = dir.path().join("system.log");
        let mut file = File::create(&path).expect("log file should be created");

        append_system_log_line_to_file(
            &mut file,
            "2026-02-22T15:02:01.123Z",
            1345,
            "first line\nsecond line",
        );

        assert_eq!(
            fs::read_to_string(path).expect("log file should be readable"),
            "2026-02-22T15:02:01.123Z pid=1345 first line\n2026-02-22T15:02:01.123Z pid=1345 second line\n"
        );
    }

    #[test]
    fn formats_current_log_timestamp_as_utc_rfc3339_milliseconds() {
        let timestamp = current_log_timestamp();

        assert!(timestamp.ends_with('Z'));
        assert_eq!(timestamp.len(), "2026-02-22T15:02:01.123Z".len());
        assert_eq!(&timestamp[4..5], "-");
        assert_eq!(&timestamp[7..8], "-");
        assert_eq!(&timestamp[10..11], "T");
        assert_eq!(&timestamp[13..14], ":");
        assert_eq!(&timestamp[16..17], ":");
        assert_eq!(&timestamp[19..20], ".");
    }

    #[test]
    fn formats_command_with_shell_quoting() {
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf 'hello world'\n".to_string(),
        ];

        assert_eq!(
            shell_split(&format_command(&command)).expect("command should parse"),
            command
        );
    }

    #[test]
    fn warns_when_log_file_is_larger_than_limit() {
        let message = log_size_warning_message(std::path::Path::new(SAMPLE_LOG_PATH), 10_485_761)
            .expect("large log file should warn");

        assert!(message.contains(SAMPLE_LOG_PATH));
        assert!(message.contains("10 MiB"));
    }

    #[test]
    fn does_not_warn_when_log_file_is_small_enough() {
        assert!(
            log_size_warning_message(std::path::Path::new(SAMPLE_LOG_PATH), 10_485_760)
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
