use reqwest::blocking::{Client, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use std::io::Read;

use crate::error::{Error, Result, error_fn};

pub(super) fn send(
    request: RequestBuilder,
    send_error: &str,
    status_error: &str,
) -> Result<Response> {
    request
        .send()
        .map_err(error_fn!(Error::network, "{}", send_error))?
        .error_for_status()
        .map_err(error_fn!(Error::network, "{}", status_error))
}

pub(super) fn get(
    client: &Client,
    url: &str,
    send_error: &str,
    status_error: &str,
) -> Result<Response> {
    send(client.get(url), send_error, status_error)
}

pub(super) fn read_bytes(mut response: Response, read_error: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    response
        .read_to_end(&mut bytes)
        .map_err(error_fn!(Error::network, "{}", read_error))?;
    Ok(bytes)
}

pub(super) fn download_bytes(
    client: &Client,
    url: &str,
    send_error: &str,
    status_error: &str,
    read_error: &str,
) -> Result<Vec<u8>> {
    let response = get(client, url, send_error, status_error)?;
    read_bytes(response, read_error)
}

pub(super) fn read_json<T>(response: Response, parse_error: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    response
        .json::<T>()
        .map_err(error_fn!(Error::network, "{}", parse_error))
}
