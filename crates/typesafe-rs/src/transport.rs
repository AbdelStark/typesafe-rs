use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode};
use url::Url;

use crate::error::{Error, TransportError};

#[derive(Debug)]
pub(crate) struct HttpTransport {
    client: reqwest::Client,
}

pub(crate) struct RawResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub attempts: u32,
}

impl HttpTransport {
    pub(crate) fn new() -> Result<Self, Error> {
        let client = reqwest::Client::builder()
            .tcp_nodelay(true)
            .pool_idle_timeout(Duration::from_secs(90))
            .http2_keep_alive_interval(Duration::from_secs(30))
            .http2_keep_alive_timeout(Duration::from_secs(10))
            .http2_keep_alive_while_idle(true)
            .http2_adaptive_window(true)
            .build()
            .map_err(|err| Error::InvalidRequest(format!("failed to build HTTP client: {err}")))?;
        Ok(Self { client })
    }

    pub(crate) async fn send(
        &self,
        method: Method,
        url: Url,
        headers: HeaderMap,
        body: Option<Bytes>,
        timeout: Duration,
    ) -> Result<RawResponse, Error> {
        let mut request = self
            .client
            .request(method, url)
            .headers(headers)
            .timeout(timeout);
        if let Some(body) = body {
            request = request.body(body);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => return Err(classify_reqwest(err, timeout)),
        };
        let status = response.status();
        let headers = response.headers().clone();
        let body = match response.bytes().await {
            Ok(body) => body,
            Err(err) => return Err(classify_reqwest(err, timeout)),
        };
        Ok(RawResponse {
            status,
            headers,
            body,
            attempts: 1,
        })
    }
}

fn classify_reqwest(err: reqwest::Error, timeout: Duration) -> Error {
    if err.is_timeout() {
        Error::Timeout { after: timeout }
    } else {
        Error::Connection(TransportError::from_reqwest(&err))
    }
}

pub(crate) fn join_endpoint(base: &Url, path: &str) -> Result<Url, Error> {
    let mut root = base.as_str().trim_end_matches('/').to_owned();
    root.push_str(path);
    Url::parse(&root).map_err(|err| Error::InvalidRequest(format!("invalid endpoint URL: {err}")))
}
