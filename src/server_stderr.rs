use crate::system_log::{log_lsp_server_stderr_line, log_unexpected_error};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

const STDERR_TAIL_LIMIT: usize = 4096;
const STDERR_FLUSH_WAIT: Duration = Duration::from_millis(50);

pub(crate) struct CapturedStderr {
    state: Arc<(Mutex<StderrState>, Condvar)>,
}

struct StderrState {
    tail: VecDeque<u8>,
    partial_line: Vec<u8>,
    finished: bool,
}

impl CapturedStderr {
    pub(crate) fn spawn<R>(mut reader: R, mirror_to_stderr: bool) -> Self
    where
        R: Read + Send + 'static,
    {
        let state = Arc::new((
            Mutex::new(StderrState {
                tail: VecDeque::new(),
                partial_line: Vec::new(),
                finished: false,
            }),
            Condvar::new(),
        ));
        let thread_state = Arc::clone(&state);
        thread::spawn(move || {
            let mut buffer = [0_u8; 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => append_stderr(&thread_state, &buffer[..read], mirror_to_stderr),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => {
                        log_unexpected_error(&format!("failed to read LSP server stderr: {error}"));
                        break;
                    }
                }
            }
            finish_stderr(&thread_state);
        });

        Self { state }
    }

    pub(crate) fn summary(&self) -> Option<String> {
        let (lock, ready) = &*self.state;
        let mut state = lock.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.finished {
            let result = ready
                .wait_timeout(state, STDERR_FLUSH_WAIT)
                .unwrap_or_else(PoisonError::into_inner);
            state = result.0;
        }

        let stderr = String::from_utf8_lossy(&state.tail.iter().copied().collect::<Vec<_>>())
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if stderr.is_empty() {
            None
        } else {
            Some(stderr)
        }
    }
}

fn append_stderr(state: &Arc<(Mutex<StderrState>, Condvar)>, chunk: &[u8], mirror_to_stderr: bool) {
    if mirror_to_stderr {
        let mut stderr = std::io::stderr().lock();
        let _write_result = stderr.write_all(chunk);
        let _flush_result = stderr.flush();
    }

    let (lock, _) = &**state;
    let mut state = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let mut completed_lines = Vec::new();

    for byte in chunk {
        state.tail.push_back(*byte);
        if state.tail.len() > STDERR_TAIL_LIMIT {
            state.tail.pop_front();
        }

        if *byte == b'\n' {
            completed_lines.push(take_line(&mut state.partial_line));
        } else {
            state.partial_line.push(*byte);
        }
    }

    drop(state);
    for line in completed_lines {
        log_lsp_server_stderr_line(&line);
    }
}

fn finish_stderr(state: &Arc<(Mutex<StderrState>, Condvar)>) {
    let (lock, ready) = &**state;
    let mut state = lock.lock().unwrap_or_else(PoisonError::into_inner);
    let final_line = if state.partial_line.is_empty() {
        None
    } else {
        Some(take_line(&mut state.partial_line))
    };
    state.finished = true;
    ready.notify_all();
    drop(state);

    if let Some(line) = final_line {
        log_lsp_server_stderr_line(&line);
    }
}

fn take_line(buffer: &mut Vec<u8>) -> String {
    let mut line = std::mem::take(buffer);
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8_lossy(&line).into_owned()
}
