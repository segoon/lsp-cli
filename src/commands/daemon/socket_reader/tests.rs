use super::*;
use crate::lsp::transport::write_message;
use serde_json::json;
use std::io::{BufRead, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const TEST_TIMEOUT: Duration = Duration::from_secs(3);

fn pair(deadline: Instant) -> (SocketReader, UnixStream) {
    let (socket, peer) = UnixStream::pair().expect("socket pair");
    peer.set_read_timeout(Some(TEST_TIMEOUT))
        .expect("peer read timeout");
    peer.set_write_timeout(Some(TEST_TIMEOUT))
        .expect("peer write timeout");
    (SocketReader::new(socket, Some(deadline)), peer)
}

fn assert_closed(peer: &mut UnixStream) {
    assert_eq!(peer.read(&mut [0]).expect("peer observes shutdown"), 0);
}

#[test]
fn silent_first_message_expires_without_coordinator_progress() {
    let (mut reader, mut peer) = pair(Instant::now() + Duration::from_millis(30));
    reader
        .next_message()
        .expect_err("silent handshake must expire");
    assert_closed(&mut peer);
}

#[test]
fn trickled_header_and_body_cannot_renew_deadline() {
    for prefix in ["Content-Length: ", "Content-Length: 10000\r\n\r\n"] {
        let (mut reader, mut peer) = pair(Instant::now() + Duration::from_millis(60));
        peer.write_all(prefix.as_bytes()).expect("partial frame");
        let (done, result) = mpsc::channel();
        let worker = thread::spawn(move || {
            done.send(reader.next_message()).expect("reader result");
        });
        let writer = thread::spawn(move || {
            while peer.write_all(b"1").is_ok() {
                thread::sleep(Duration::from_millis(5));
            }
            assert_closed(&mut peer);
        });
        result
            .recv_timeout(TEST_TIMEOUT)
            .expect("trickling did not postpone expiry")
            .expect_err("incomplete first frame must expire");
        worker.join().expect("reader exits");
        writer.join().expect("writer observes shutdown");
    }
}

#[test]
fn expiry_wins_over_a_buffered_first_message() {
    let (mut reader, mut peer) = pair(Instant::now() + TEST_TIMEOUT);
    write_message(&mut peer, &json!({"id": 1})).expect("first frame");
    // Prefill the exact production buffer, then expire the deadline without relying on sleep.
    assert!(!reader.reader.fill_buf().expect("prefill").is_empty());
    reader.reader.get_mut().deadline = Some(Instant::now());
    reader
        .next_message()
        .expect_err("buffered frame still needs deadline validation");
    assert_closed(&mut peer);
}

#[test]
fn first_message_preserves_pipelined_frames_and_clears_read_timeout() {
    let (mut reader, mut peer) = pair(Instant::now() + TEST_TIMEOUT);
    let mut frames = Vec::new();
    for id in 1..=3 {
        write_message(&mut frames, &json!({"id": id})).expect("encode frame");
    }
    peer.write_all(&frames).expect("pipeline");
    for id in 1..=3 {
        assert_eq!(
            reader.next_message().expect("frame"),
            Some(json!({"id": id}))
        );
        assert!(reader.reader.get_ref().deadline.is_none());
        assert_eq!(
            reader
                .reader
                .get_ref()
                .socket
                .read_timeout()
                .expect("socket timeout"),
            None
        );
    }
}
