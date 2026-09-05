use super::super::events::{Event, Source};
use super::super::protocol::stop_request;
use super::super::session_tests::Fixture;
use super::super::{ClientPhase, LifecycleState};
use super::*;
use crate::lsp::transport::read_message;
use crate::lsp::transport::write_message;
use serde_json::json;
use std::io::{BufReader, Read, Write};

const TEST_TIMEOUT: Duration = Duration::from_secs(3);

struct Peer {
    reader: BufReader<UnixStream>,
}

impl Peer {
    fn new(daemon: &mut Daemon, accepted_at: Instant) -> Self {
        let (socket, peer) = UnixStream::pair().expect("connection pair");
        peer.set_read_timeout(Some(TEST_TIMEOUT))
            .expect("read timeout");
        daemon
            .dispatch(Event::Accepted {
                stream: socket,
                accepted_at,
            })
            .expect("accept connection");
        Self {
            reader: BufReader::new(peer),
        }
    }

    fn send(&mut self, message: &Value) {
        write_message(self.reader.get_mut(), message).expect("send message");
    }

    fn response(&mut self) -> Value {
        read_message(&mut self.reader)
            .expect("read response")
            .expect("response exists")
    }

    fn assert_closed(&mut self) {
        match self.reader.read(&mut [0]) {
            Ok(bytes) => assert_eq!(bytes, 0, "connection closed without a response"),
            Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset),
        }
    }
}

impl Fixture {
    fn idle() -> Self {
        let mut fixture = Self::new();
        fixture
            .daemon
            .disconnect_client()
            .expect("disconnect initial client");
        fixture
    }

    fn connect(&mut self) -> Peer {
        Peer::new(&mut self.daemon, Instant::now())
    }

    fn initialize_message(&self) -> Value {
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "rootUri": self.daemon.target.root_uri, "capabilities": {}
        }})
    }

    fn latest_pending(&self) -> u64 {
        *self
            .daemon
            .pending_connections
            .last_key_value()
            .expect("pending connection")
            .0
    }

    fn expire_all(&mut self) {
        self.daemon
            .expire_pending_connections(Instant::now() + HANDSHAKE_TIMEOUT);
    }

    fn pump_until(&mut self, finished: impl Fn(&Daemon) -> bool) {
        for _ in 0..100 {
            if finished(&self.daemon) {
                return;
            }
            self.step();
        }
        assert!(finished(&self.daemon), "coordinator made progress");
    }
}

#[test]
fn silent_pending_peers_do_not_block_active_requests_or_stop() {
    let mut fixture = Fixture::new();
    fixture.initialize(false);
    let _silent = fixture.connect();
    let mut partial = fixture.connect();
    partial
        .reader
        .get_mut()
        .write_all(b"Content-Length: 100\r\n\r\n{")
        .expect("partial frame");
    for id in 10..20 {
        fixture.send(&json!({"jsonrpc": "2.0", "id": id, "method": "latency/echo", "params": id}));
        fixture.step();
        fixture.step();
        assert_eq!(fixture.read()["result"], id);
    }
    let mut control = fixture.connect();
    control.send(&stop_request());
    fixture.pump_until(|daemon| daemon.stop_requested);
    assert_eq!(control.response()["result"], Value::Null);
    assert!(
        fixture.daemon.active_client.is_some(),
        "stop did not need an active-client disconnect"
    );
}

#[test]
fn pending_slot_does_not_reserve_active_session_and_pipelining_survives_promotion() {
    let mut fixture = Fixture::idle();
    let _silent_first = fixture.connect();
    let mut ready_second = fixture.connect();
    let generation = fixture.latest_pending();
    let initialize = fixture.initialize_message();
    let mut frames = Vec::new();
    for message in [
        initialize,
        json!({"method": "initialized", "params": {}}),
        json!({"id": 2, "method": "latency/echo", "params": "pipeline"}),
    ] {
        write_message(&mut frames, &message).expect("encode pipeline");
    }
    ready_second
        .reader
        .get_mut()
        .write_all(&frames)
        .expect("single pipelined write");
    // Three client frames and two upstream replies, regardless of interleaving.
    for _ in 0..5 {
        fixture.step();
    }
    fixture.pump_until(|daemon| {
        daemon.active_client.as_ref().is_some_and(|client| {
            matches!(client.phase, ClientPhase::Ready)
                && client.forwarded_client_requests.is_empty()
        })
    });
    assert_eq!(
        fixture
            .daemon
            .active_client
            .as_ref()
            .expect("promoted")
            .generation,
        generation
    );
    assert!(ready_second.response().get("result").is_some());
    assert_eq!(ready_second.response()["result"], "pipeline");
    assert_eq!(fixture.daemon.pending_connections.len(), 1);
}

#[test]
fn second_initializer_gets_busy_without_disrupting_active_client() {
    let mut fixture = Fixture::idle();
    let mut first = fixture.connect();
    let mut second = fixture.connect();
    let initialize = fixture.initialize_message();
    first.send(&initialize);
    fixture.pump_until(|daemon| daemon.active_client.is_some());
    let admitted = fixture
        .daemon
        .active_client
        .as_ref()
        .expect("first client")
        .generation;
    second.send(&initialize);
    fixture.pump_until(|daemon| daemon.pending_connections.is_empty());
    assert_eq!(
        second.response()["error"]["message"],
        "another daemon client is already connected"
    );
    second.assert_closed();
    assert_eq!(
        fixture
            .daemon
            .active_client
            .as_ref()
            .expect("first remains")
            .generation,
        admitted
    );
}

#[test]
fn capacity_rejects_newcomers_preserves_existing_slots_and_recovers() {
    let mut fixture = Fixture::idle();
    let mut peers: Vec<_> = (0..MAX_PENDING_CONNECTIONS)
        .map(|_| fixture.connect())
        .collect();
    let generations: Vec<_> = fixture.daemon.pending_connections.keys().copied().collect();
    let (socket, mut stream) = UnixStream::pair().expect("excess control connection");
    stream
        .set_read_timeout(Some(TEST_TIMEOUT))
        .expect("control read timeout");
    write_message(&mut stream, &stop_request()).expect("queued stop request");
    fixture
        .daemon
        .dispatch(Event::Accepted {
            stream: socket,
            accepted_at: Instant::now(),
        })
        .expect("reject excess control connection");
    let mut newcomer = Peer {
        reader: BufReader::new(stream),
    };
    newcomer.assert_closed();
    assert_eq!(
        fixture
            .daemon
            .pending_connections
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        generations
    );
    assert!(fixture.daemon.active_client.is_none());
    // The rejected socket is closed before a stop request (or any first message) can be read.
    assert!(!fixture.daemon.stop_requested);
    fixture.expire_all();
    for peer in &mut peers {
        peer.assert_closed();
    }
    let mut control = fixture.connect();
    control.send(&stop_request());
    fixture.pump_until(|daemon| daemon.stop_requested);
    assert_eq!(control.response()["result"], Value::Null);
}

#[test]
fn eof_frees_capacity_without_waiting_for_the_deadline() {
    let mut fixture = Fixture::idle();
    let mut peers: Vec<_> = (0..MAX_PENDING_CONNECTIONS)
        .map(|_| fixture.connect())
        .collect();
    let peer = peers.pop().expect("last peer");
    peer.reader
        .get_ref()
        .shutdown(std::net::Shutdown::Write)
        .expect("peer closes input");
    fixture.pump_until(|daemon| daemon.pending_connections.len() < MAX_PENDING_CONNECTIONS);
    let _replacement = fixture.connect();
    assert_eq!(
        fixture.daemon.pending_connections.len(),
        MAX_PENDING_CONNECTIONS
    );
}

#[test]
fn deadline_includes_accept_queue_time_and_ignores_late_events() {
    let mut fixture = Fixture::idle();
    let idle_since = fixture.daemon.idle_since;
    let mut expired = Peer::new(
        &mut fixture.daemon,
        Instant::now()
            .checked_sub(HANDSHAKE_TIMEOUT)
            .expect("past acceptance time"),
    );
    expired.assert_closed();
    assert!(fixture.daemon.pending_connections.is_empty());
    let mut pending = fixture.connect();
    let retired = fixture.latest_pending();
    fixture.expire_all();
    pending.assert_closed();
    let _replacement = fixture.connect();
    let replacement = fixture.latest_pending();
    for event in [
        ReaderEvent::Message(fixture.initialize_message().into()),
        ReaderEvent::EndOfStream,
        ReaderEvent::Error("expired".into()),
    ] {
        fixture
            .daemon
            .dispatch(Event::Reader(Source::Client(retired), event))
            .expect("ignore retired event");
    }
    assert!(fixture.daemon.active_client.is_none());
    assert!(
        fixture
            .daemon
            .pending_connections
            .contains_key(&replacement)
    );
    assert_eq!(fixture.daemon.idle_since, idle_since);
}

#[test]
fn expiry_wins_over_first_message_and_runs_during_other_events() {
    let mut fixture = Fixture::idle();
    let mut peer = fixture.connect();
    let generation = fixture.latest_pending();
    fixture
        .daemon
        .pending_connections
        .get_mut(&generation)
        .expect("pending")
        .deadline = Instant::now();
    fixture
        .daemon
        .dispatch(Event::Reader(
            Source::Client(generation),
            ReaderEvent::Message(fixture.initialize_message().into()),
        ))
        .expect("expired first message");
    peer.assert_closed();
    assert!(fixture.daemon.active_client.is_none());
    let mut peer = fixture.connect();
    let generation = fixture.latest_pending();
    fixture
        .daemon
        .pending_connections
        .get_mut(&generation)
        .expect("pending")
        .deadline = Instant::now();
    fixture
        .daemon
        .dispatch(Event::Reader(
            Source::Upstream(999),
            ReaderEvent::Message(json!({"method": "notification"}).into()),
        ))
        .expect("continuous unrelated traffic still checks expiry");
    peer.assert_closed();
    assert!(fixture.daemon.pending_connections.is_empty());
}

#[test]
fn invalid_first_messages_are_isolated_to_pending_connection() {
    let mut fixture = Fixture::idle();
    for message in [
        json!({"id": 1, "method": "initialize", "params": {"rootUri": "file:///elsewhere/"}}),
        json!({"id": 1, "method": "initialize"}),
        json!({"id": 1, "method": "latency/echo"}),
    ] {
        let mut peer = fixture.connect();
        peer.send(&message);
        fixture.pump_until(|daemon| daemon.pending_connections.is_empty());
        assert!(peer.response().get("error").is_some());
        peer.assert_closed();
        assert!(fixture.daemon.active_client.is_none());
    }
    for bytes in [
        b"invalid-header\r\n\r\n".as_slice(),
        b"Content-Length: 2\r\n\r\n{}",
    ] {
        let mut peer = fixture.connect();
        peer.reader
            .get_mut()
            .write_all(bytes)
            .expect("invalid first message");
        fixture.pump_until(|daemon| daemon.pending_connections.is_empty());
        peer.assert_closed();
        assert!(!fixture.daemon.stop_requested);
    }
    let mut control = fixture.connect();
    control.send(&stop_request());
    fixture.pump_until(|daemon| daemon.stop_requested);
    assert_eq!(control.response()["id"], "lsp-cli/stop");
}

#[test]
fn pending_deadlines_compete_with_idle_and_disappear_after_admission() {
    let mut fixture = Fixture::idle();
    fixture.daemon.idle_timeout = Duration::from_secs(10);
    let _pending = fixture.connect();
    let deadline = fixture
        .daemon
        .pending_connections
        .values()
        .next()
        .expect("pending")
        .deadline;
    let now = Instant::now();
    assert_eq!(
        fixture.daemon.next_event_timeout(now),
        Some(deadline.saturating_duration_since(now))
    );
    fixture.daemon.idle_timeout = Duration::ZERO;
    assert_eq!(fixture.daemon.next_event_timeout(now), Some(Duration::ZERO));
    fixture.expire_all();
    fixture.daemon.idle_timeout = Duration::from_secs(10);
    let mut peer = fixture.connect();
    peer.send(&fixture.initialize_message());
    fixture.pump_until(|daemon| daemon.active_client.is_some());
    assert_eq!(fixture.daemon.next_event_timeout(Instant::now()), None);
}

#[test]
fn stop_and_error_teardown_close_all_pending_readers() {
    for normal_stop in [false, true] {
        let mut fixture = Fixture::new();
        fixture.initialize(false);
        let mut silent = fixture.connect();
        let mut partial = fixture.connect();
        partial
            .reader
            .get_mut()
            .write_all(b"Content-Len")
            .expect("partial header");
        if normal_stop {
            fixture.daemon.begin_stop().expect("begin normal stop");
            fixture.pump_until(|daemon| daemon.lifecycle == LifecycleState::Stopped);
            assert!(fixture.daemon.pending_connections.is_empty());
        }
        // Explicit destruction also exercises error-exit cleanup with live pending readers.
        drop(fixture);
        silent.assert_closed();
        partial.assert_closed();
    }
}

#[test]
fn pending_connections_do_not_prevent_idle_exit() {
    let mut fixture = Fixture::idle();
    fixture.daemon.upstream.take();
    let mut peer = fixture.connect();
    fixture.daemon.idle_timeout = Duration::ZERO;
    fixture
        .daemon
        .serve()
        .expect("idle exit with pending connection");
    assert!(fixture.daemon.pending_connections.is_empty());
    peer.assert_closed();
}

#[test]
fn event_loop_wakes_to_expire_pending_client_while_active_client_remains() {
    use std::sync::mpsc;
    use std::thread;
    let mut fixture = Fixture::new();
    fixture.initialize(false);
    let mut pending = fixture.connect();
    let generation = fixture.latest_pending();
    fixture
        .daemon
        .pending_connections
        .get_mut(&generation)
        .expect("pending")
        .deadline = Instant::now() + Duration::from_millis(20);
    // A generous watchdog below the worker's two-second deadline distinguishes coordinator
    // wakeup from the independent reader timeout, without measuring forwarding latency.
    pending
        .reader
        .get_ref()
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("expiry watchdog");
    let mut active = fixture.peer.get_ref().try_clone().expect("active writer");
    let (done, result) = mpsc::channel();
    let coordinator = thread::spawn(move || {
        done.send(fixture.daemon.serve()).expect("serve result");
    });
    // This closure comes from the coordinator's timer, before the reader's two-second budget.
    pending.assert_closed();
    write_message(&mut active, &stop_request()).expect("stop on still-active connection");
    result
        .recv_timeout(TEST_TIMEOUT)
        .expect("coordinator remained responsive")
        .expect("clean stop");
    coordinator.join().expect("coordinator exits");
}
