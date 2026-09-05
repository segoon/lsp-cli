use super::*;

fn worker(messages: usize, bytes: usize, flagged_since: Option<Instant>) -> WriterWorker {
    let (sender, _) = mpsc::channel();
    let (acknowledge, _) = mpsc::channel();
    WriterWorker {
        sender: Some(sender),
        progress: Arc::new(Progress {
            messages: AtomicUsize::new(messages),
            bytes: AtomicUsize::new(bytes),
        }),
        flagged_since,
        next_id: 0,
        socket: None,
        cancelled: Arc::new(AtomicBool::new(false)),
        acknowledge,
        thread: None,
    }
}

#[test]
fn queue_flag_tracks_both_limits_and_clears_below_them() {
    let now = Instant::now();
    let mut writer = worker(MESSAGE_LIMIT, 1, None);
    assert_eq!(writer.outstanding(), (MESSAGE_LIMIT, 1));
    writer.refresh_flag(now);
    assert_eq!(
        writer.deadline(Duration::from_secs(2)),
        Some(now + Duration::from_secs(2))
    );

    writer
        .progress
        .messages
        .store(MESSAGE_LIMIT - 1, Ordering::Release);
    writer.progress.bytes.store(BYTE_LIMIT, Ordering::Release);
    writer.refresh_flag(now);
    assert!(writer.deadline(Duration::ZERO).is_some());

    writer
        .progress
        .bytes
        .store(BYTE_LIMIT - 1, Ordering::Release);
    writer.refresh_flag(now);
    assert_eq!(writer.deadline(Duration::ZERO), None);
}

#[test]
fn persistent_flag_times_out_at_configured_duration() {
    let started = Instant::now();
    let mut writer = worker(MESSAGE_LIMIT, 0, Some(started));
    assert!(!writer.timed_out(
        started + Duration::from_millis(1999),
        Duration::from_secs(2)
    ));
    assert!(writer.timed_out(started + Duration::from_secs(2), Duration::from_secs(2)));
}
