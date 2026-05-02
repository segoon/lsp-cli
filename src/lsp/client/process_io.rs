use std::collections::VecDeque;
use std::io::{BufReader, Read};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use super::IncomingMessage;
use crate::lsp::transport::{log_debug_message, read_message};

const STDERR_TAIL_LIMIT: usize = 4096;
const STDERR_FLUSH_WAIT: Duration = Duration::from_millis(50);

pub(super) struct CapturedStderr {
    state: Arc<(Mutex<StderrState>, Condvar)>,
}

struct StderrState {
    tail: VecDeque<u8>,
    finished: bool,
}

impl CapturedStderr {
    pub(super) fn spawn<R>(mut reader: R) -> Self
    where
        R: Read + Send + 'static,
    {
        let state = Arc::new((
            Mutex::new(StderrState {
                tail: VecDeque::new(),
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
                    Ok(read) => append_stderr(&thread_state, &buffer[..read]),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let (lock, ready) = &*thread_state;
            let mut state = lock.lock().expect("stderr state should lock");
            state.finished = true;
            ready.notify_all();
        });

        Self { state }
    }

    pub(super) fn summary(&self) -> Option<String> {
        let (lock, ready) = &*self.state;
        let mut state = lock.lock().expect("stderr state should lock");
        if !state.finished {
            let result = ready
                .wait_timeout(state, STDERR_FLUSH_WAIT)
                .expect("stderr wait should succeed");
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

pub(super) fn spawn_reader<R>(reader: R, debug: bool) -> Receiver<IncomingMessage>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || reader_loop(reader, &sender, debug));
    receiver
}

fn append_stderr(state: &Arc<(Mutex<StderrState>, Condvar)>, chunk: &[u8]) {
    let (lock, _) = &**state;
    let mut state = lock.lock().expect("stderr state should lock");
    for byte in chunk {
        state.tail.push_back(*byte);
        if state.tail.len() > STDERR_TAIL_LIMIT {
            state.tail.pop_front();
        }
    }
}

fn reader_loop<R>(reader: R, sender: &Sender<IncomingMessage>, debug: bool)
where
    R: Read,
{
    let mut reader = BufReader::new(reader);

    loop {
        match read_message(&mut reader) {
            Ok(Some(message)) => {
                log_debug_message(debug, "<- ", &message);
                if sender.send(IncomingMessage::Message(message)).is_err() {
                    return;
                }
            }
            Ok(None) => {
                let _ = sender.send(IncomingMessage::EndOfStream);
                return;
            }
            Err(error) => {
                let _ = sender.send(IncomingMessage::Error(error));
                return;
            }
        }
    }
}
