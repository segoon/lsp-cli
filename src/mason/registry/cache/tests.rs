use std::cell::RefCell;
use std::fmt::Write as _;
use std::net::TcpListener;
use std::thread;

use super::*;

const RELEASE: &str = r#"{"tag_name":"2026-09-28","assets":[]}"#;

struct FixtureResponse {
    status: u16,
    headers: &'static [(&'static str, &'static str)],
    body: &'static str,
}

fn serve(responses: Vec<FixtureResponse>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture server should bind");
    let address = listener
        .local_addr()
        .expect("fixture server address should resolve");
    let server = thread::spawn(move || {
        responses
            .into_iter()
            .map(|response| {
                let (mut stream, _) = listener.accept().expect("fixture request should arrive");
                let request = read_request(&mut stream);
                let reason = match response.status {
                    200 => "OK",
                    403 => "Forbidden",
                    429 => "Too Many Requests",
                    _ => "Test",
                };
                let mut head = format!(
                    "HTTP/1.1 {} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    response.status,
                    response.body.len()
                );
                for (name, value) in response.headers {
                    write!(head, "{name}: {value}\r\n")
                        .expect("fixture response header should format");
                }
                head.push_str("\r\n");
                stream
                    .write_all(head.as_bytes())
                    .and_then(|()| stream.write_all(response.body.as_bytes()))
                    .expect("fixture response should be written");
                request
            })
            .collect()
    });
    (format!("http://{address}/release"), server)
}

fn read_request(stream: &mut impl Read) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let read = stream
            .read(&mut buffer)
            .expect("fixture request should be readable");
        assert!(read > 0, "fixture request ended before its headers");
        request.extend_from_slice(
            buffer
                .get(..read)
                .expect("read byte count should fit the fixture buffer"),
        );
    }
    String::from_utf8(request).expect("fixture request should be UTF-8")
}

fn fetch(
    responses: Vec<FixtureResponse>,
    token: Option<&str>,
) -> (Result<GithubRelease>, Vec<Duration>, Vec<String>) {
    let (url, server) = serve(responses);
    let delays = RefCell::new(Vec::new());
    let result = fetch_latest_release_from(&Client::new(), &url, token, |delay| {
        delays.borrow_mut().push(delay);
    });
    (
        result,
        delays.into_inner(),
        server.join().expect("fixture server should finish"),
    )
}

fn response(status: u16, body: &'static str) -> FixtureResponse {
    FixtureResponse {
        status,
        headers: &[],
        body,
    }
}

#[test]
fn retries_429_and_rate_limit_messages_with_exponential_backoff() {
    let (result, delays, requests) = fetch(
        vec![
            response(429, "slow down"),
            response(403, r#"{"message":"API rate limit exceeded"}"#),
            response(200, RELEASE),
        ],
        Some("fixture-token"),
    );

    assert_eq!(
        result
            .expect("rate-limited request should eventually succeed")
            .tag_name,
        "2026-09-28"
    );
    assert_eq!(delays, [Duration::from_secs(1), Duration::from_secs(2)]);
    assert!(requests.iter().all(|request| {
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-token")
    }));
}

#[test]
fn caps_retries_and_reports_the_final_rate_limit() {
    let (result, delays, requests) = fetch(
        vec![
            response(429, "rate limited"),
            response(429, "rate limited"),
            response(429, "rate limited"),
            response(429, "rate limited"),
        ],
        None,
    );

    let error = result.expect_err("four rate limits should exhaust retries");
    assert!(error.contains("after 4 attempts"));
    assert_eq!(
        delays,
        [
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
        ]
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.to_ascii_lowercase().contains("authorization:"))
    );
}

#[test]
fn shorter_numeric_retry_after_reduces_the_delay() {
    let (result, delays, _) = fetch(
        vec![
            FixtureResponse {
                status: 429,
                headers: &[("Retry-After", "0")],
                body: "slow down",
            },
            response(200, RELEASE),
        ],
        None,
    );

    result.expect("request should succeed after Retry-After");
    assert_eq!(delays, [Duration::ZERO]);
}

#[test]
fn does_not_retry_an_unrelated_forbidden_response() {
    let (result, delays, requests) = fetch(vec![response(403, "forbidden")], None);

    let error = result.expect_err("unrelated forbidden response should fail");
    assert!(error.contains("failed to fetch Mason registry metadata"));
    assert!(!error.contains("after 4 attempts"));
    assert!(delays.is_empty());
    assert_eq!(requests.len(), 1);
}
