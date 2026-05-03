use std::io::{BufReader, Read};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::IncomingMessage;
use crate::lsp::transport::{log_debug_message, read_message};

pub(super) fn spawn_reader<R>(reader: R, debug: bool) -> Receiver<IncomingMessage>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || reader_loop(reader, &sender, debug));
    receiver
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
