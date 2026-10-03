use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use reqwest::blocking::Client;

use super::*;

const PAYLOAD: &str = "complete payload";

enum Reply {
    Http(u16, &'static str, &'static [(&'static str, &'static str)]),
    Interrupted,
    Disconnect,
}

fn response(status: u16) -> Reply {
    Reply::Http(status, PAYLOAD, &[])
}

struct Fixture {
    url: String,
    server: thread::JoinHandle<usize>,
}

impl Fixture {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
        listener
            .set_nonblocking(true)
            .expect("fixture should be nonblocking");
        let address = listener
            .local_addr()
            .expect("fixture should have an address");
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut count = 0;
            for reply in replies {
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "fixture request did not arrive");
                            thread::sleep(Duration::from_millis(1));
                        }
                        Err(error) => panic!("fixture accept failed: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("read timeout should set");
                read_headers(&mut stream);
                match reply {
                    Reply::Http(status, body, headers) => {
                        let mut head = format!(
                            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n",
                            body.len()
                        );
                        for (name, value) in headers {
                            write!(head, "{name}: {value}\r\n").expect("header should format");
                        }
                        stream
                            .write_all(format!("{head}\r\n{body}").as_bytes())
                            .expect("response should write");
                    }
                    Reply::Interrupted => {
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\nConnection: close\r\n\r\npartial").expect("partial response should write");
                    }
                    Reply::Disconnect => {}
                }
                count += 1;
            }
            count
        });
        Self {
            url: format!("http://{address}/download"),
            server,
        }
    }

    fn fetch(self, policy: &RetryPolicy) -> (Result<Vec<u8>, Failure>, Vec<Duration>, usize) {
        let delays = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&delays);
        let client = Client::builder()
            .no_proxy()
            .retry(reqwest::retry::never())
            .build()
            .expect("client should build");
        let result = policy.run(
            client.get(&self.url),
            |response| {
                response
                    .bytes()
                    .map(|bytes| bytes.to_vec())
                    .map_err(Failure::from)
            },
            move |delay| {
                recorded.lock().expect("delays should lock").push(delay);
            },
        );
        let count = self.server.join().expect("fixture should finish");
        let delays = delays.lock().expect("delays should lock").clone();
        (result, delays, count)
    }
}

fn read_headers(stream: &mut TcpStream) {
    let mut headers = Vec::new();
    let mut byte = [0];
    while !headers.ends_with(b"\r\n\r\n") {
        stream
            .read_exact(&mut byte)
            .expect("request headers should arrive");
        headers.extend_from_slice(&byte);
    }
}

fn policy() -> RetryPolicy {
    RetryPolicy {
        backoff: ExponentialBuilder::default().with_min_delay(Duration::from_secs(2)),
        ..RetryPolicy::default()
    }
}

#[test]
fn retries_every_server_error_status() {
    for status in 500..=599 {
        let (result, delays, count) =
            Fixture::new(vec![response(status), response(200)]).fetch(&policy());
        assert_eq!(
            result.expect("5xx should recover"),
            PAYLOAD.as_bytes(),
            "status {status}"
        );
        assert_eq!(delays, [Duration::from_secs(2)]);
        assert_eq!(count, 2);
    }
}

#[test]
fn retries_network_failures_and_discards_partial_response_bodies() {
    for reply in [Reply::Disconnect, Reply::Interrupted] {
        let (result, delays, count) = Fixture::new(vec![reply, response(200)]).fetch(&policy());
        assert_eq!(
            result.expect("network failure should recover"),
            PAYLOAD.as_bytes()
        );
        assert_eq!(delays, [Duration::from_secs(2)]);
        assert_eq!(count, 2);
    }
}

#[test]
fn persistent_failure_exhausts_exponential_retries() {
    let (result, delays, count) =
        Fixture::new((0..4).map(|_| response(503)).collect()).fetch(&policy());
    let error = result
        .expect_err("persistent server failure should fail")
        .to_string();
    assert!(error.contains("503 Test"));
    assert!(error.contains("after 4 attempts"));
    assert_eq!(
        delays,
        [
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8)
        ]
    );
    assert_eq!(count, 4);
}

#[test]
fn client_errors_are_permanent_except_timeouts_and_rate_limits() {
    for status in [400, 401, 403, 404, 422] {
        let (result, delays, count) = Fixture::new(vec![response(status)]).fetch(&policy());
        result.expect_err("permanent HTTP failure should fail");
        assert!(delays.is_empty());
        assert_eq!(count, 1);
    }
    for status in [408, 429] {
        let (result, delays, count) =
            Fixture::new(vec![response(status), response(200)]).fetch(&policy());
        result.expect("temporary HTTP failure should recover");
        assert_eq!(delays, [Duration::from_secs(2)]);
        assert_eq!(count, 2);
    }
}

#[test]
fn retry_after_is_a_minimum_wait() {
    let (result, delays, count) = Fixture::new(vec![
        Reply::Http(503, "unavailable", &[("Retry-After", "10")]),
        response(200),
    ])
    .fetch(&policy());
    result.expect("server should recover after requested delay");
    assert_eq!(delays, [Duration::from_secs(10)]);
    assert_eq!(count, 2);
}

#[test]
fn retry_after_exceeding_budget_fails_without_waiting_or_repeating() {
    let (result, delays, count) = Fixture::new(vec![Reply::Http(
        503,
        "unavailable",
        &[("Retry-After", "301")],
    )])
    .fetch(&policy());
    let error = result.expect_err("retry cannot fit").to_string();
    assert!(error.contains("503 Test"));
    assert!(error.contains("time budget"));
    assert!(delays.is_empty());
    assert_eq!(count, 1);
}

#[test]
fn connection_refusal_is_retried() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("unused address should bind");
    let url = format!(
        "http://{}/",
        listener.local_addr().expect("address should resolve")
    );
    // Release the reserved port to exercise a real connection error rather than a server response.
    drop(listener);
    let delays = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&delays);
    let client = Client::builder()
        .no_proxy()
        .build()
        .expect("client should build");
    let result = policy().run(
        client.get(url),
        |_| Ok(()),
        move |delay| recorded.lock().expect("delays should lock").push(delay),
    );
    assert!(
        result
            .expect_err("connection should fail")
            .to_string()
            .contains("after 4 attempts")
    );
    assert_eq!(
        *delays.lock().expect("delays should lock"),
        [
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8)
        ]
    );
}

#[test]
fn expired_budget_does_not_start_a_request() {
    let policy = RetryPolicy {
        budget: Duration::ZERO,
        ..policy()
    };
    let result = policy.run(
        Client::new().get("http://127.0.0.1:1"),
        |_| Ok(()),
        |_| panic!("expired transfer must not sleep"),
    );
    assert!(
        result
            .expect_err("expired transfer should fail")
            .to_string()
            .contains("time budget")
    );
}

#[test]
fn stalled_response_cannot_outlive_the_transfer_budget() {
    // Keep the listening socket open without serving headers, so the request must time out.
    let listener = TcpListener::bind("127.0.0.1:0").expect("stalled server should bind");
    let url = format!(
        "http://{}/",
        listener.local_addr().expect("address should resolve")
    );
    let policy = RetryPolicy {
        budget: Duration::from_millis(30),
        attempt_timeout: Duration::from_secs(1),
        ..policy()
    };
    let client = Client::builder()
        .no_proxy()
        .build()
        .expect("client should build");
    let started = Instant::now();
    let result = policy.run(
        client.get(url),
        |_| Ok(()),
        |_| panic!("retry cannot fit the deadline"),
    );
    assert!(
        result
            .expect_err("stalled transfer should fail")
            .to_string()
            .contains("time budget")
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn invalid_request_is_not_treated_as_a_network_failure() {
    let result = policy().run(
        Client::new().get("://invalid"),
        |_| Ok(()),
        |_| panic!("invalid URL must not be retried"),
    );
    let error = result.expect_err("invalid URL should fail before sending");
    assert!(!error.retryable);
    assert!(!error.to_string().contains("cannot be replayed"));
}
