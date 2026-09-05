use super::*;
use serde_json::json;
use std::sync::{Condvar, Mutex};
use std::time::Instant;

#[derive(Default)]
struct SinkState {
    entered: bool,
    released: bool,
    records: Vec<String>,
    dropped: Vec<usize>,
}

struct PausedDestination {
    state: Arc<(Mutex<SinkState>, Condvar)>,
}

impl LogDestination for PausedDestination {
    fn write(&mut self, record: LogRecord) {
        let (lock, ready) = &*self.state;
        let mut state = lock.lock().expect("sink state");
        state.entered = true;
        ready.notify_all();
        while !state.released {
            state = ready.wait(state).expect("release sink");
        }
        state.records.push(match record {
            LogRecord::Debug { prefix, message } => {
                format!("{prefix}{}", serialize_debug_message(&message))
            }
            LogRecord::System(message) => message,
            LogRecord::Stderr(chunk) => String::from_utf8_lossy(&chunk).into_owned(),
        });
    }

    fn report_dropped(&mut self, count: usize) {
        self.state.0.lock().expect("sink state").dropped.push(count);
    }
}

fn paused_worker(queue_limit: usize) -> (LoggerWorker, Arc<(Mutex<SinkState>, Condvar)>) {
    let state = Arc::new((Mutex::new(SinkState::default()), Condvar::new()));
    let worker = LoggerWorker::spawn_with(
        true,
        Box::new(PausedDestination {
            state: Arc::clone(&state),
        }),
        queue_limit,
    )
    .expect("logger worker");
    (worker, state)
}

fn release(state: &Arc<(Mutex<SinkState>, Condvar)>) {
    state.0.lock().expect("sink state").released = true;
    state.1.notify_all();
}

fn wait_until_entered(state: &Arc<(Mutex<SinkState>, Condvar)>) {
    let mut guard = state.0.lock().expect("sink state");
    while !guard.entered {
        guard = state.1.wait(guard).expect("worker enters sink");
    }
}

#[test]
fn full_queue_drops_new_records_and_reports_after_progress() {
    let (mut worker, state) = paused_worker(1);
    let logger = worker.logger();
    logger.system("blocked");
    wait_until_entered(&state);
    logger.system("queued");
    logger.system("dropped");
    assert_eq!(logger.dropped(), 1);
    release(&state);
    worker.finish(Duration::from_secs(1));
    let state = state.0.lock().expect("sink state");
    assert_eq!(state.records, ["blocked", "queued"]);
    assert_eq!(state.dropped.iter().sum::<usize>(), 1);
}

#[test]
fn debug_serialization_runs_after_enqueue() {
    let (mut worker, state) = paused_worker(2);
    worker
        .logger()
        .debug("prefix: ", Arc::new(json!({"answer": 42})));
    wait_until_entered(&state);
    assert!(state.0.lock().expect("sink state").records.is_empty());
    release(&state);
    worker.finish(Duration::from_secs(1));
    assert_eq!(
        state.0.lock().expect("sink state").records,
        ["prefix: {\n  \"answer\": 42\n}"]
    );
}

#[test]
fn shutdown_wait_is_bounded_when_destination_is_blocked() {
    let (mut worker, state) = paused_worker(1);
    worker.logger().system("blocked");
    wait_until_entered(&state);
    let started = Instant::now();
    worker.finish(Duration::from_millis(20));
    assert!(started.elapsed() < Duration::from_millis(100));
    release(&state);
    worker.finish(Duration::from_secs(1));
}
