use crate::error::{Error, Result};
use crate::lsp::transport::read_message;
use serde_json::Value;
use std::io::{self, BufReader, Read};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::time::Instant;

const DEADLINE_MESSAGE: &str = "daemon client did not send its first message before the deadline";

pub(super) struct SocketReader {
    reader: BufReader<DeadlineSocket>,
}

impl SocketReader {
    pub(super) fn new(socket: UnixStream, deadline: Option<Instant>) -> Self {
        Self {
            reader: BufReader::new(DeadlineSocket { socket, deadline }),
        }
    }

    pub(super) fn next_message(&mut self) -> Result<Option<Value>> {
        let message = read_message(&mut self.reader);
        let socket = self.reader.get_mut();
        if let Some(deadline) = socket.deadline {
            // BufReader may satisfy reads from its buffer, so also check the deadline after
            // the entire first frame has been parsed. Keep this buffer when admitting a client.
            let finish = if Instant::now() >= deadline {
                Err(Error::lsp(DEADLINE_MESSAGE))
            } else {
                socket.socket.set_read_timeout(None).map_err(|error| {
                    Error::unexpected(format!(
                        "failed to clear daemon client handshake timeout: {error}"
                    ))
                })
            };
            if finish.is_err() || !matches!(&message, Ok(Some(_))) {
                // Close independently of coordinator progress, which may be in synchronous I/O.
                let _ = socket.socket.shutdown(Shutdown::Both);
            }
            finish?;
            socket.deadline = None;
        }
        message
    }
}

struct DeadlineSocket {
    socket: UnixStream,
    deadline: Option<Instant>,
}

impl Read for DeadlineSocket {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Some(deadline) = self.deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, DEADLINE_MESSAGE));
            }
            // Socket timeouts apply per read. Recompute the remaining absolute budget so
            // partial headers/bodies and trickled bytes cannot renew the handshake deadline.
            self.socket.set_read_timeout(Some(remaining))?;
        }
        self.socket.read(buffer)
    }
}

#[cfg(test)]
mod tests;
