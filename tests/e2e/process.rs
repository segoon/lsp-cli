use std::ffi::OsStr;
use std::io;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use command_group::{CommandGroup as _, GroupChild};
use wait_timeout::ChildExt as _;

const OUTPUT_EXCERPT_LIMIT: usize = 16 * 1024;
const READER_CLEANUP_DEADLINE: Duration = Duration::from_secs(1);

pub(crate) struct ProcessOutput {
    details: ProcessDetails,
    status: ExitStatus,
}

pub(crate) struct ProcessFailure {
    details: Box<ProcessDetails>,
    reason: String,
}

struct ProcessDetails {
    command: String,
    cwd: PathBuf,
    deadline: Duration,
    elapsed: Duration,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

struct RunningGroup {
    child: Option<GroupChild>,
    stdout_reader: OutputReader,
    stderr_reader: OutputReader,
}

struct OutputReader {
    result: Option<Receiver<io::Result<Vec<u8>>>>,
    thread: Option<JoinHandle<()>>,
    output: Option<Vec<u8>>,
}

pub(crate) fn run(
    command: &mut Command,
    deadline: Duration,
) -> Result<ProcessOutput, ProcessFailure> {
    let command_line = render_command(command);
    let cwd = command
        .get_current_dir()
        .map_or_else(|| PathBuf::from("<inherited>"), PathBuf::from);
    let started = Instant::now();
    let absolute_deadline = started.checked_add(deadline).unwrap_or(started);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = command.group_spawn().map_err(|error| ProcessFailure {
        details: Box::new(ProcessDetails::empty(
            command_line.clone(),
            cwd.clone(),
            deadline,
            started.elapsed(),
        )),
        reason: format!("failed to start process group: {error}"),
    })?;
    let mut running = RunningGroup::new(child).map_err(|(mut child, error)| {
        let cleanup = terminate_group(&mut child);
        ProcessFailure {
            details: Box::new(ProcessDetails::empty(
                command_line.clone(),
                cwd.clone(),
                deadline,
                started.elapsed(),
            )),
            reason: format!("failed to capture process output: {error}; {cleanup}"),
        }
    })?;

    // `wait-timeout` extends `Child`, so borrow the group leader only for the bounded wait.
    // If it expires, `GroupChild::kill` still terminates the complete process group.
    let wait_result = running.child_mut().and_then(|child| {
        child
            .inner()
            .wait_timeout(deadline.saturating_sub(started.elapsed()))
            .map_err(|error| format!("failed while waiting for process group: {error}"))
    });

    match wait_result {
        Ok(Some(status)) => {
            let (stdout, stderr) = match running.finish_readers_before(absolute_deadline) {
                Ok(output) => output,
                Err(reason) => {
                    let cleanup = running.terminate();
                    let (stdout, stderr) = running.finish_readers_after_termination();
                    return Err(ProcessFailure {
                        details: Box::new(ProcessDetails {
                            command: command_line,
                            cwd,
                            deadline,
                            elapsed: started.elapsed(),
                            stdout,
                            stderr,
                        }),
                        reason: format!("{reason}; {cleanup}"),
                    });
                }
            };
            running.disarm_child();
            Ok(ProcessOutput {
                details: ProcessDetails {
                    command: command_line,
                    cwd,
                    deadline,
                    elapsed: started.elapsed(),
                    stdout,
                    stderr,
                },
                status,
            })
        }
        Ok(None) => {
            let cleanup = running.terminate();
            let (stdout, stderr) = running.finish_readers_after_termination();
            Err(ProcessFailure {
                details: Box::new(ProcessDetails {
                    command: command_line,
                    cwd,
                    deadline,
                    elapsed: started.elapsed(),
                    stdout,
                    stderr,
                }),
                reason: format!("process exceeded its deadline; {cleanup}"),
            })
        }
        Err(reason) => {
            let cleanup = running.terminate();
            let (stdout, stderr) = running.finish_readers_after_termination();
            Err(ProcessFailure {
                details: Box::new(ProcessDetails {
                    command: command_line,
                    cwd,
                    deadline,
                    elapsed: started.elapsed(),
                    stdout,
                    stderr,
                }),
                reason: format!("{reason}; {cleanup}"),
            })
        }
    }
}

impl ProcessOutput {
    pub(crate) fn status(&self) -> ExitStatus {
        self.status
    }

    pub(crate) fn stdout(&self) -> &[u8] {
        &self.details.stdout
    }

    pub(crate) fn stderr(&self) -> &[u8] {
        &self.details.stderr
    }

    pub(crate) fn diagnostic(&self, reason: &str, runtime_state: &str) -> String {
        self.details
            .diagnostic(reason, Some(self.status), runtime_state)
    }
}

impl ProcessFailure {
    pub(crate) fn diagnostic(&self, runtime_state: &str) -> String {
        self.details.diagnostic(&self.reason, None, runtime_state)
    }
}

impl ProcessDetails {
    fn empty(command: String, cwd: PathBuf, deadline: Duration, elapsed: Duration) -> Self {
        Self {
            command,
            cwd,
            deadline,
            elapsed,
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    fn diagnostic(&self, reason: &str, status: Option<ExitStatus>, runtime_state: &str) -> String {
        let status = status.map_or_else(|| "not available".to_string(), |value| value.to_string());
        format!(
            "{reason}\ncommand: {}\nworking directory: {}\nstatus: {status}\nelapsed: {:?}\ndeadline: {:?}\nruntime state:\n{runtime_state}\nstdout:\n{}\nstderr:\n{}",
            self.command,
            self.cwd.display(),
            self.elapsed,
            self.deadline,
            output_excerpt(&self.stdout),
            output_excerpt(&self.stderr),
        )
    }
}

impl RunningGroup {
    fn new(mut child: GroupChild) -> Result<Self, (GroupChild, io::Error)> {
        let Some(stdout) = child.inner().stdout.take() else {
            return Err((child, io::Error::other("stdout pipe was not created")));
        };
        let Some(stderr) = child.inner().stderr.take() else {
            return Err((child, io::Error::other("stderr pipe was not created")));
        };

        Ok(Self {
            child: Some(child),
            stdout_reader: OutputReader::new(stdout),
            stderr_reader: OutputReader::new(stderr),
        })
    }

    fn child_mut(&mut self) -> Result<&mut GroupChild, String> {
        self.child
            .as_mut()
            .ok_or_else(|| "running process group lost its child handle".to_string())
    }

    fn disarm_child(&mut self) {
        let _child = self.child.take();
    }

    fn terminate(&mut self) -> String {
        let Some(mut child) = self.child.take() else {
            return "process group already reaped".to_string();
        };
        terminate_group(&mut child)
    }

    fn finish_readers_before(&mut self, deadline: Instant) -> Result<(Vec<u8>, Vec<u8>), String> {
        let stdout = self.stdout_reader.finish_before(deadline, "stdout");
        let stderr = self.stderr_reader.finish_before(deadline, "stderr");
        match (stdout, stderr) {
            (Ok(()), Ok(())) => Ok(self.take_output()),
            (Err(stdout), Ok(())) => Err(stdout),
            (Ok(()), Err(stderr)) => Err(stderr),
            (Err(stdout), Err(stderr)) => Err(format!("{stdout}; {stderr}")),
        }
    }

    fn finish_readers_after_termination(&mut self) -> (Vec<u8>, Vec<u8>) {
        let deadline = Instant::now()
            .checked_add(READER_CLEANUP_DEADLINE)
            .unwrap_or_else(Instant::now);
        let error = self.finish_readers_before(deadline).err();
        let (stdout, mut stderr) = self.take_output();
        if let Some(error) = error {
            stderr.extend_from_slice(format!("\nfailed to capture output: {error}").as_bytes());
        }
        (stdout, stderr)
    }

    fn take_output(&mut self) -> (Vec<u8>, Vec<u8>) {
        (
            self.stdout_reader.take_output(),
            self.stderr_reader.take_output(),
        )
    }
}

impl Drop for RunningGroup {
    fn drop(&mut self) {
        // This guard is necessary because test assertions may unwind while descendants are alive.
        if self.child.is_some() {
            let _cleanup_result = self.terminate();
        }
        let _captured_output = self.finish_readers_after_termination();
    }
}

impl OutputReader {
    fn new(reader: impl io::Read + Send + 'static) -> Self {
        let (sender, result) = mpsc::channel();
        let thread = thread::spawn(move || {
            let _send_result = sender.send(read_all(reader));
        });
        Self {
            result: Some(result),
            thread: Some(thread),
            output: None,
        }
    }

    fn finish_before(&mut self, deadline: Instant, stream: &str) -> Result<(), String> {
        if self.output.is_some() {
            return Ok(());
        }
        let Some(result) = self.result.as_ref() else {
            return Err(format!("{stream} capture result is unavailable"));
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        let output = match result.recv_timeout(remaining) {
            Ok(output) => output.map_err(|error| format!("failed to read {stream}: {error}"))?,
            Err(RecvTimeoutError::Timeout) => {
                return Err(format!("{stream} remained open after the command deadline"));
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(format!("{stream} capture thread exited without a result"));
            }
        };
        self.result = None;
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            return Err(format!("{stream} capture thread panicked"));
        }
        self.output = Some(output);
        Ok(())
    }

    fn take_output(&mut self) -> Vec<u8> {
        self.output.take().unwrap_or_default()
    }
}

fn terminate_group(child: &mut GroupChild) -> String {
    let kill_result = child.kill();
    let wait_result = child.wait();
    match (kill_result, wait_result) {
        (Ok(()), Ok(status)) => format!("process group killed and reaped with {status}"),
        (Err(kill_error), Ok(status)) if kill_error.kind() == io::ErrorKind::InvalidInput => {
            format!("process group exited during cleanup with {status}")
        }
        (Err(kill_error), Ok(status)) => {
            format!("process group reaped with {status}, but kill failed: {kill_error}")
        }
        (Ok(()), Err(wait_error)) => {
            format!("process group was killed but could not be reaped: {wait_error}")
        }
        (Err(kill_error), Err(wait_error)) => {
            format!("process group could not be killed ({kill_error}) or reaped ({wait_error})")
        }
    }
}

fn read_all(mut reader: impl io::Read) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;
    Ok(output)
}

fn render_command(command: &Command) -> String {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(render_argument)
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_argument(argument: &OsStr) -> String {
    format!("{argument:?}")
}

fn output_excerpt(output: &[u8]) -> String {
    let omitted = output.len().saturating_sub(OUTPUT_EXCERPT_LIMIT);
    let excerpt = output
        .get(omitted..)
        .map_or(output, |bounded_output| bounded_output);
    if omitted == 0 {
        String::from_utf8_lossy(excerpt).into_owned()
    } else {
        format!(
            "<{} earlier bytes omitted>\n{}",
            omitted,
            String::from_utf8_lossy(excerpt)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{OUTPUT_EXCERPT_LIMIT, output_excerpt};

    #[test]
    fn diagnostic_output_keeps_only_a_bounded_tail() {
        let output = vec![b'x'; OUTPUT_EXCERPT_LIMIT + 7];
        let excerpt = output_excerpt(&output);

        assert!(excerpt.starts_with("<7 earlier bytes omitted>\n"));
        assert_eq!(
            excerpt.bytes().filter(|byte| *byte == b'x').count(),
            OUTPUT_EXCERPT_LIMIT
        );
    }
}
