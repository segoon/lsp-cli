use reqwest::blocking::{Client, RequestBuilder};
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

#[path = "http/retry.rs"]
pub(crate) mod retry;
use retry::{Failure, RetryPolicy};

pub(crate) fn download_bytes(client: &Client, url: &str, send_error: &str) -> Result<Vec<u8>> {
    RetryPolicy::default()
        .run(
            client.get(url),
            |response| {
                response
                    .bytes()
                    .map(|bytes| bytes.to_vec())
                    .map_err(Failure::from)
            },
            std::thread::sleep,
        )
        .map_err(|error| Error::network(format!("{send_error}: {error}")))
}

pub(crate) fn read_json<T: DeserializeOwned>(
    request: RequestBuilder,
    context: &str,
    parse_context: &str,
    policy: &RetryPolicy,
    sleep: impl Fn(std::time::Duration) + 'static,
) -> Result<T> {
    let bytes = policy
        .run(
            request,
            |response| response.bytes().map_err(Failure::from),
            sleep,
        )
        .map_err(|error| Error::network(format!("{context}: {error}")))?;
    // Invalid JSON is a metadata defect, not a failed network transfer.
    serde_json::from_slice(&bytes)
        .map_err(|error| Error::network(format!("{parse_context}: {error}")))
}
