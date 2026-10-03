use std::cell::Cell;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use backon::{BackoffBuilder, BlockingRetryable, ExponentialBuilder};
use reqwest::StatusCode;
use reqwest::blocking::{RequestBuilder, Response};
use reqwest::header::RETRY_AFTER;

/// One policy owns retries for both the request and its complete response body.
/// Keeping HTTP errors typed here avoids retrying checksum, JSON, or filesystem defects.
pub(crate) struct RetryPolicy {
    pub(crate) backoff: ExponentialBuilder,
    pub(crate) budget: Duration,
    pub(crate) attempt_timeout: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            backoff: ExponentialBuilder::default()
                .with_min_delay(Duration::from_secs(2))
                .with_max_times(3)
                .with_jitter(),
            budget: Duration::from_secs(300),
            attempt_timeout: Duration::from_secs(120),
        }
    }
}

impl RetryPolicy {
    pub(crate) fn run<T>(
        &self,
        request: RequestBuilder,
        mut consume: impl FnMut(Response) -> Result<T, Failure>,
        sleep: impl Fn(Duration) + 'static,
    ) -> Result<T, Failure> {
        let (client, request) = request.build_split();
        let request = request.map_err(Failure::from)?;
        let started = Instant::now();
        let attempts = Cell::new(0);
        // Backon's iterator requires Send + Sync, even for this blocking operation.
        // Atomics let its delay adapter share the most recent HTTP retry hints.
        let retry_after = AtomicU64::new(0);
        let budget_exhausted = AtomicBool::new(false);
        // Adapt the crate's backoff rather than adding a second retry loop. Retry-After
        // is a minimum wait; if it cannot fit, return the original failure immediately.
        let backoff = self
            .backoff
            .build()
            .map(|delay| delay.max(Duration::from_secs(retry_after.load(Ordering::Relaxed))))
            .take_while(|delay| {
                let fits = *delay < self.budget.saturating_sub(started.elapsed());
                budget_exhausted.store(!fits, Ordering::Relaxed);
                fits
            });
        let result = (|| {
            let remaining = self.budget.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(Failure::permanent("HTTP transfer time budget exhausted"));
            }
            let Some(mut attempt) = request.try_clone() else {
                return Err(Failure::permanent(
                    "HTTP download request cannot be replayed",
                ));
            };
            attempts.set(attempts.get() + 1);
            *attempt.timeout_mut() = Some(remaining.min(self.attempt_timeout));
            let response = client.execute(attempt).map_err(Failure::from)?;
            let response = checked_response(response)?;
            consume(response)
        })
        .retry(backoff)
        .when(|error: &Failure| {
            retry_after.store(error.retry_after.as_secs(), Ordering::Relaxed);
            error.retryable
        })
        .notify(|error, delay| {
            eprintln!(
                "warning: HTTP transfer failed: {error}; retrying in {:.1}s after attempt {}",
                delay.as_secs_f64(),
                attempts.get()
            );
        })
        .sleep(sleep)
        .call();
        result.map_err(|mut error| {
            if attempts.get() > 1 {
                error.message = format!("{} after {} attempts", error.message, attempts.get());
            }
            if budget_exhausted.load(Ordering::Relaxed) {
                error
                    .message
                    .push_str("; next retry exceeds the HTTP transfer time budget");
            }
            error
        })
    }
}

fn checked_response(response: Response) -> Result<Response, Failure> {
    let status = response.status();
    let retry_after = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_default();
    let Err(error) = response.error_for_status_ref() else {
        return Ok(response);
    };
    let mut retryable = status.is_server_error()
        || matches!(
            status,
            StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS
        );
    if status == StatusCode::FORBIDDEN {
        let body = response.text().map_err(Failure::from)?;
        retryable = body.to_ascii_lowercase().contains("rate limit exceeded");
    }
    Err(Failure {
        message: error.to_string(),
        retryable,
        retry_after,
    })
}

#[derive(Debug)]
pub(crate) struct Failure {
    message: String,
    retryable: bool,
    retry_after: Duration,
}

impl Failure {
    pub(crate) fn network(error: impl fmt::Display) -> Self {
        Self {
            message: error.to_string(),
            retryable: true,
            retry_after: Duration::ZERO,
        }
    }

    pub(crate) fn permanent(error: impl fmt::Display) -> Self {
        Self {
            retryable: false,
            ..Self::network(error)
        }
    }
}

impl From<reqwest::Error> for Failure {
    fn from(error: reqwest::Error) -> Self {
        if error.is_builder() {
            Self::permanent(error)
        } else {
            Self::network(error)
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[cfg(test)]
#[path = "retry/tests.rs"]
mod tests;
