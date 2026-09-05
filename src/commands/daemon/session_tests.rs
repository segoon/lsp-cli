use super::*;
use crate::lsp::transport::read_message;
use crate::test_support::{TestDir, with_env_vars};
use serde_json::json;
use std::io::BufReader;
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use std::sync::mpsc;

const TIMEOUT: Duration = Duration::from_secs(3);

pub(super) struct Fixture {
    pub(super) daemon: Daemon,
    pub(super) peer: BufReader<UnixStream>,
    _dir: TestDir,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let dir = TestDir::new("daemon-session");
        let target = tests::daemon_target(&dir);
        let mut events = EventQueue::new();
        let upstream = fake_upstream(&mut events);
        let (socket, peer) = UnixStream::pair().expect("client pair");
        peer.set_read_timeout(Some(TIMEOUT)).expect("read timeout");
        let client = ClientSession::new(socket, &mut events, None).expect("client");
        Self {
            daemon: Daemon {
                accept_worker: None,
                events,
                socket_owned: true,
                target,
                debug: false,
                idle_timeout: TIMEOUT,
                upstream: Some(upstream),
                active_client: Some(client),
                pending_connections: BTreeMap::new(),
                orphaned_client_requests: BTreeSet::new(),
                idle_since: Instant::now(),
                stop_requested: false,
            },
            peer: BufReader::new(peer),
            _dir: dir,
        }
    }

    pub(super) fn send(&mut self, message: &Value) {
        write_message(self.peer.get_mut(), message).expect("client write");
    }

    pub(super) fn step(&mut self) {
        let delivery = self
            .daemon
            .events
            .receive(Some(TIMEOUT))
            .expect("coordinator event");
        let result = self.daemon.dispatch(delivery.event);
        let _ = delivery.acknowledge.send(());
        result.expect("dispatch event");
    }

    pub(super) fn read(&mut self) -> Value {
        read_message(&mut self.peer)
            .expect("read response")
            .expect("response")
    }

    pub(super) fn initialize(&mut self, reused: bool) -> Value {
        self.send(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "rootUri": self.daemon.target.root_uri, "capabilities": {}
            }}),
        );
        self.step();
        if !reused {
            self.step();
        }
        let response = self.read();
        self.send(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
        self.step();
        response
    }

    pub(super) fn replace_client(&mut self) {
        self.daemon.disconnect_client().expect("disconnect");
        let (socket, peer) = UnixStream::pair().expect("replacement pair");
        peer.set_read_timeout(Some(TIMEOUT)).expect("read timeout");
        self.daemon.active_client = Some(
            ClientSession::new(socket, &mut self.daemon.events, None).expect("replacement client"),
        );
        self.peer = BufReader::new(peer);
    }
}

fn fake_upstream(events: &mut EventQueue) -> UpstreamServer {
    let mut child = with_env_vars(&[], || {
        Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/daemon_latency.py"
            ))
            .arg("--server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("fake server")
    });
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let stderr = CapturedStderr::spawn(child.stderr.take().expect("stderr"), false);
    let generation = events.next_generation().expect("upstream generation");
    let reader =
        ReaderWorker::spawn(stdout, Source::Upstream(generation), events).expect("upstream reader");
    UpstreamServer {
        child,
        stdin,
        stderr,
        generation,
        reader,
        initialize_fingerprint: None,
        initialize_result: None,
        restart_required: false,
        background_work: BackgroundWorkTracker::default(),
    }
}

#[test]
fn forwards_sequential_and_pipelined_requests_with_warm_reuse() {
    let mut fixture = Fixture::new();
    let initialized = fixture.initialize(false);
    for width in [1, 8] {
        for id in 10..10 + width {
            fixture
                .send(&json!({"jsonrpc": "2.0", "id": id, "method": "latency/echo", "params": id}));
        }
        for _ in 0..width * 2 {
            fixture.step();
        }
        for id in 10..10 + width {
            assert_eq!(
                fixture.read(),
                json!({"jsonrpc": "2.0", "id": id, "result": id})
            );
        }
    }
    fixture.replace_client();
    assert_eq!(
        fixture.initialize(true),
        initialized,
        "reuse cached initialization and same process"
    );
}

#[test]
fn stale_generations_cannot_modify_replacement_sessions() {
    let mut fixture = Fixture::new();
    fixture.initialize(false);
    let old_client = fixture
        .daemon
        .active_client
        .as_ref()
        .expect("client")
        .generation;
    let old_upstream = fixture
        .daemon
        .upstream
        .as_ref()
        .expect("upstream")
        .generation;
    fixture.replace_client();
    fixture.daemon.upstream.take();
    fixture.daemon.upstream = Some(fake_upstream(&mut fixture.daemon.events));
    let client = fixture
        .daemon
        .active_client
        .as_ref()
        .expect("client")
        .generation;
    let upstream = fixture
        .daemon
        .upstream
        .as_ref()
        .expect("upstream")
        .generation;
    for source in [Source::Client(old_client), Source::Upstream(old_upstream)] {
        for event in [
            ReaderEvent::Message(json!({"id": 9, "method": "exit"})),
            ReaderEvent::EndOfStream,
            ReaderEvent::Error("retired".into()),
        ] {
            fixture
                .daemon
                .dispatch(Event::Reader(source, event))
                .expect("ignore stale event");
            assert_eq!(
                fixture
                    .daemon
                    .active_client
                    .as_ref()
                    .expect("client retained")
                    .generation,
                client
            );
            assert_eq!(
                fixture
                    .daemon
                    .upstream
                    .as_ref()
                    .expect("upstream retained")
                    .generation,
                upstream
            );
        }
    }
    assert!(!fixture.daemon.stop_requested);
}

#[test]
fn shutdown_preserves_unrelated_client_message_for_dispatch() {
    let mut fixture = Fixture::new();
    fixture.initialize(false);
    fixture.send(&json!({"jsonrpc": "2.0", "method": "exit"}));
    fixture
        .daemon
        .shutdown_upstream()
        .expect("shutdown reply through event queue");
    assert!(fixture.daemon.upstream.is_none());
    fixture.step();
    assert!(
        fixture.daemon.active_client.is_none(),
        "queued client exit was preserved"
    );
}

#[test]
fn idle_deadline_runs_during_continuous_events() {
    let mut fixture = Fixture::new();
    fixture.daemon.disconnect_client().expect("disconnect");
    fixture.daemon.upstream.take();
    fixture.daemon.idle_timeout = Duration::from_millis(20);
    fixture.daemon.idle_since = Instant::now();
    let (socket, mut peer) = UnixStream::pair().expect("notification pair");
    let producer = ReaderWorker::socket(socket, Source::Upstream(999), &fixture.daemon.events)
        .expect("producer");
    let writer = thread::spawn(move || {
        while write_message(&mut peer, &json!({"method": "notification"})).is_ok() {}
    });
    let (done, result) = mpsc::channel();
    let daemon_thread = thread::spawn(move || {
        done.send(fixture.daemon.serve()).expect("serve result");
    });
    result
        .recv_timeout(TIMEOUT)
        .expect("idle deadline was not starved")
        .expect("idle stop");
    producer.cancel();
    writer.join().expect("notification writer exits");
    daemon_thread.join().expect("daemon exits");
}

#[test]
fn idle_receive_wakes_for_connection_then_client_stop() {
    let mut fixture = Fixture::new();
    fixture.daemon.disconnect_client().expect("disconnect");
    let path = fixture.daemon.target.socket_path.clone();
    let listener = UnixListener::bind(&path).expect("listener");
    fixture.daemon.accept_worker =
        Some(AcceptWorker::spawn(listener, &path, &fixture.daemon.events).expect("accept worker"));
    let (done, result) = mpsc::channel();
    let daemon_thread = thread::spawn(move || {
        done.send(fixture.daemon.serve()).expect("serve result");
    });
    assert!(matches!(
        stop_socket(&path, false).expect("stop while idle"),
        StopSocketResult::Stopped
    ));
    result
        .recv_timeout(TIMEOUT)
        .expect("stop handled")
        .expect("clean stop");
    daemon_thread.join().expect("daemon exits");
    assert!(!path.exists());
}

#[test]
fn dropping_stopped_daemon_preserves_replacement_socket() {
    let Fixture {
        mut daemon,
        _dir: dir,
        ..
    } = Fixture::new();
    let path = daemon.target.socket_path.clone();
    let listener = UnixListener::bind(&path).expect("listener");
    daemon.accept_worker =
        Some(AcceptWorker::spawn(listener, &path, &daemon.events).expect("accept worker"));
    daemon.upstream.take();
    daemon.stop().expect("stop removes owned socket");
    let _replacement = UnixListener::bind(&path).expect("replacement listener");
    // Explicit teardown reproduces another daemon binding between normal stop and destruction.
    drop(daemon);
    assert!(path.exists(), "replacement socket must remain linked");
    assert!(
        dir.path().exists(),
        "keep temporary directory alive through teardown"
    );
}
