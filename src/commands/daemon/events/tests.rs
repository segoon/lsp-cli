use super::*;
use crate::lsp::transport::write_message;
use crate::test_support::TestDir;
use serde_json::{Value, json};
use std::io::Write;

const TIMEOUT: Duration = Duration::from_secs(3);

struct Fixture {
    events: EventQueue,
    workers: Vec<ReaderWorker>,
    peers: Vec<UnixStream>,
}

impl Fixture {
    fn new(sources: &[Source]) -> Self {
        let events = EventQueue::new();
        let mut workers = Vec::new();
        let mut peers = Vec::new();
        for source in sources {
            let (reader, peer) = UnixStream::pair().expect("socket pair");
            workers.push(ReaderWorker::socket(reader, *source, &events).expect("reader worker"));
            peers.push(peer);
        }
        Self {
            events,
            workers,
            peers,
        }
    }

    fn send(&mut self, index: usize, value: &Value) {
        write_message(self.peers.get_mut(index).expect("peer"), value).expect("write frame");
    }

    fn receive(&mut self) -> Delivery {
        self.events
            .receive(Some(TIMEOUT))
            .expect("event before timeout")
    }
}

fn message(delivery: Delivery, source: Source) -> Value {
    let Event::Reader(actual_source, ReaderEvent::Message(value)) = delivery.event else {
        panic!("expected message");
    };
    assert_eq!(actual_source, source);
    delivery.acknowledge.send(()).expect("acknowledge");
    Arc::unwrap_or_clone(value)
}

#[test]
fn reader_admission_preserves_order_and_opposite_direction_opportunities() {
    let client = Source::Client(1);
    let upstream = Source::Upstream(2);
    let mut fixture = Fixture::new(&[client, upstream]);
    for number in 0..20 {
        fixture.send(0, &json!(number));
    }
    let first = fixture.receive();
    assert!(matches!(
        fixture.events.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    fixture.send(1, &json!("reply"));
    assert_eq!(message(fixture.receive(), upstream), json!("reply"));
    assert_eq!(message(first, client), json!(0));
    for number in 1..20 {
        assert_eq!(message(fixture.receive(), client), json!(number));
    }
}

#[test]
fn cancellation_releases_unacknowledged_reader() {
    let mut fixture = Fixture::new(&[Source::Client(1)]);
    fixture.send(0, &json!(1));
    let delivery = fixture.receive();
    let mut worker = fixture.workers.pop().expect("worker");
    worker.cancel();
    worker
        .thread
        .take()
        .expect("reader thread")
        .join()
        .expect("cancelled reader exits");
    assert!(delivery.acknowledge.send(()).is_err());
}

#[test]
fn cancellation_interrupts_partial_frame_reads() {
    for partial in ["", "Content-Len", "Content-Length: 100\r\n\r\n{"] {
        let mut fixture = Fixture::new(&[Source::Client(1)]);
        fixture
            .peers
            .first_mut()
            .expect("peer")
            .write_all(partial.as_bytes())
            .expect("partial frame");
        let mut worker = fixture.workers.pop().expect("worker");
        worker.cancel();
        worker
            .thread
            .take()
            .expect("reader thread")
            .join()
            .expect("cancelled reader exits");
    }
}

#[test]
fn reader_reports_eof_and_malformed_frames() {
    for (bytes, malformed) in [("", false), ("bad-header\r\n\r\n", true)] {
        let source = Source::Client(1);
        let mut fixture = Fixture::new(&[source]);
        let peer = fixture.peers.first_mut().expect("peer");
        peer.write_all(bytes.as_bytes()).expect("frame");
        peer.shutdown(Shutdown::Write).expect("close writer");
        let delivery = fixture.receive();
        assert!(matches!(delivery.event, Event::Reader(actual, _) if actual == source));
        assert_eq!(
            matches!(delivery.event, Event::Reader(_, ReaderEvent::Error(_))),
            malformed
        );
        assert_eq!(
            matches!(delivery.event, Event::Reader(_, ReaderEvent::EndOfStream)),
            !malformed
        );
        delivery
            .acknowledge
            .send(())
            .expect("acknowledge terminal event");
    }
}

#[test]
fn blocking_receive_wakes_on_reader_event() {
    let mut fixture = Fixture::new(&[Source::Client(1)]);
    let mut events = fixture.events;
    let (done, result) = mpsc::channel();
    let receiver = thread::spawn(move || {
        let delivery = events.receive(None).expect("blocking receive");
        done.send(message(delivery, Source::Client(1)))
            .expect("result");
    });
    write_message(fixture.peers.first_mut().expect("peer"), &json!("wake")).expect("frame");
    assert_eq!(
        result.recv_timeout(TIMEOUT).expect("receiver woke"),
        json!("wake")
    );
    receiver.join().expect("receiver exits");
}

#[test]
fn listener_bounds_accepted_sockets_and_cancels_in_both_wait_states() {
    for pending in [false, true] {
        let dir = TestDir::new("event-accept");
        let path = dir.path().join("daemon.sock");
        let listener = UnixListener::bind(&path).expect("listener");
        let mut events = EventQueue::new();
        let worker = AcceptWorker::spawn(listener, &path, &events).expect("accept worker");
        let lifetime = Arc::downgrade(&worker.worker.cancelled);
        if pending {
            let _first_peer = UnixStream::connect(&path).expect("first connection");
            let first = events.receive(Some(TIMEOUT)).expect("first accepted");
            let _second_peer = UnixStream::connect(&path).expect("second connection");
            assert!(matches!(
                events.receiver.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            assert!(matches!(first.event, Event::Accepted { .. }));
            first.acknowledge.send(()).expect("admit second socket");
            let second = events.receive(Some(TIMEOUT)).expect("second accepted");
            assert!(matches!(second.event, Event::Accepted { .. }));
            // Teardown must release admission even while this delivery remains unacknowledged.
            drop(worker);
            assert!(second.acknowledge.send(()).is_err());
        } else {
            // Teardown must wake a listener that has no incoming connection.
            drop(worker);
        }
        assert!(
            lifetime.upgrade().is_none(),
            "accept worker exited and was joined"
        );
    }
}

#[test]
fn generations_never_wrap_into_a_retired_identity() {
    let mut events = EventQueue::new();
    assert_ne!(
        events.next_generation().expect("first"),
        events.next_generation().expect("second")
    );
    events.generation = u64::MAX;
    events
        .next_generation()
        .expect_err("generation exhaustion must not wrap");
}
