use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use crate::types::ResponseMeta;

/// SDK error taxonomy, aligned with the official TypeScript classes.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No API key in config or `TYPESAFE_API_KEY`.
    #[error(
        "No API key was provided. Pass `api_key` to ClientConfig or set the TYPESAFE_API_KEY environment variable."
    )]
    MissingApiKey,
    /// Client-side validation failed before a network call.
    #[error("{0}")]
    InvalidRequest(String),
    /// DNS, TLS, or connection failure (`APIConnectionError`).
    #[error("Connection error: {0}")]
    Connection(#[source] TransportError),
    /// The attempt exceeded the configured timeout (`APITimeoutError`).
    #[error("Request timed out after {}ms.", after.as_millis())]
    Timeout {
        /// Timeout budget that elapsed.
        after: Duration,
    },
    /// Non-2xx HTTP response after retries are exhausted.
    #[error(transparent)]
    Api(Box<ApiError>),
    /// Response body was not valid JSON for the expected type.
    #[error("failed to decode response: {source}")]
    Decode {
        /// Serde error.
        #[source]
        source: serde_json::Error,
        /// Raw response bytes (never shown in [`Display`](fmt::Display)).
        body: Bytes,
        /// HTTP metadata from the failing attempt.
        meta: Box<ResponseMeta>,
    },
    /// JSON parsed but did not have the documented shape.
    #[error("Unexpected response shape from {endpoint}")]
    UnexpectedShape {
        /// Endpoint that returned the unexpected body.
        endpoint: &'static str,
        /// HTTP metadata from the failing attempt.
        meta: Box<ResponseMeta>,
    },
}

impl Error {
    /// `x-typesafe-request-id` when this error came from an HTTP response.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api(err) => err.request_id.as_deref(),
            Self::Decode { meta, .. } | Self::UnexpectedShape { meta, .. } => {
                meta.request_id.as_deref()
            }
            _ => None,
        }
    }

    /// HTTP status when this error came from an HTTP response.
    #[must_use]
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Self::Api(err) => Some(err.status),
            Self::Decode { meta, .. } | Self::UnexpectedShape { meta, .. } => meta.status,
            _ => None,
        }
    }

    /// HTTP attempts performed before this error was returned.
    #[must_use]
    pub fn attempts(&self) -> Option<u32> {
        match self {
            Self::Api(err) => Some(err.attempts),
            Self::Decode { meta, .. } | Self::UnexpectedShape { meta, .. } => Some(meta.attempts),
            _ => None,
        }
    }

    /// The API error when this is [`Self::Api`].
    #[must_use]
    pub fn as_api(&self) -> Option<&ApiError> {
        match self {
            Self::Api(err) => Some(err),
            _ => None,
        }
    }

    /// HTTP status class when this is an API error.
    #[must_use]
    pub fn kind(&self) -> Option<ApiErrorKind> {
        self.as_api().map(|err| err.kind)
    }

    /// True when the API returned HTTP 429.
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        self.kind() == Some(ApiErrorKind::RateLimit)
    }

    /// True when the API returned HTTP 401.
    #[must_use]
    pub fn is_auth(&self) -> bool {
        self.kind() == Some(ApiErrorKind::Authentication)
    }

    /// True when the attempt timed out.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// True when a transport/connection failure occurred.
    #[must_use]
    pub fn is_connection(&self) -> bool {
        matches!(self, Self::Connection(_))
    }
}

impl From<ApiError> for Error {
    fn from(err: ApiError) -> Self {
        Self::Api(Box::new(err))
    }
}

/// Sanitized transport failure. Display never includes API keys.
#[derive(Debug, Clone)]
pub struct TransportError {
    message: String,
    pre_send: bool,
}

impl TransportError {
    pub(crate) fn from_reqwest(err: &reqwest::Error) -> Self {
        Self {
            message: redact_secrets(&err.to_string()),
            pre_send: err.is_connect(),
        }
    }

    /// True when the failure happened before request bytes were written.
    #[must_use]
    pub fn is_pre_send(&self) -> bool {
        self.pre_send
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TransportError {}

/// Non-2xx API response.
#[derive(Debug)]
pub struct ApiError {
    /// HTTP status.
    pub status: StatusCode,
    /// Classification of the status.
    pub kind: ApiErrorKind,
    /// Parsed error body (not included in [`Display`](fmt::Display) in full).
    pub body: ErrorBody,
    /// `x-typesafe-request-id` when present.
    pub request_id: Option<String>,
    /// Response headers.
    pub headers: HeaderMap,
    /// Path that was called, e.g. `/v1/systemone`.
    pub endpoint: String,
    /// Total HTTP attempts including the original request.
    pub attempts: u32,
}

impl ApiError {
    pub(crate) fn from_response(
        status: StatusCode,
        body: ErrorBody,
        headers: HeaderMap,
        endpoint: &str,
        attempts: u32,
    ) -> Self {
        let request_id = crate::headers::request_id(&headers);
        Self {
            status,
            kind: ApiErrorKind::from_status(status),
            body,
            request_id,
            headers,
            endpoint: endpoint.to_owned(),
            attempts,
        }
    }

    fn short_message(&self) -> Option<String> {
        let raw = match &self.body {
            ErrorBody::Json(value) => extract_message(value),
            ErrorBody::Text(text) if !text.is_empty() => Some(text.clone()),
            ErrorBody::Text(_) | ErrorBody::Empty => None,
        }?;
        let redacted = redact_secrets(&raw);
        if redacted.len() > 200 {
            Some(format!("{}…", &redacted[..200]))
        } else {
            Some(redacted)
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:?}", self.status.as_u16(), self.kind)?;
        if let Some(id) = &self.request_id {
            write!(f, " [request-id: {id}]")?;
        }
        if let Some(msg) = self.short_message() {
            write!(f, ": {msg}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// HTTP status class, matching the official SDK error subclasses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApiErrorKind {
    /// HTTP 400.
    BadRequest,
    /// HTTP 401.
    Authentication,
    /// HTTP 403.
    PermissionDenied,
    /// HTTP 404.
    NotFound,
    /// HTTP 409.
    Conflict,
    /// HTTP 422.
    UnprocessableEntity,
    /// HTTP 429.
    RateLimit,
    /// HTTP 5xx (including 529).
    InternalServer,
    /// Any other non-2xx status.
    Other,
}

impl ApiErrorKind {
    /// Classify an HTTP status code.
    #[must_use]
    pub fn from_status(status: StatusCode) -> Self {
        match status.as_u16() {
            400 => Self::BadRequest,
            401 => Self::Authentication,
            403 => Self::PermissionDenied,
            404 => Self::NotFound,
            409 => Self::Conflict,
            422 => Self::UnprocessableEntity,
            429 => Self::RateLimit,
            500..=599 => Self::InternalServer,
            _ => Self::Other,
        }
    }
}

/// Body of an error response.
#[derive(Clone, Debug, PartialEq)]
pub enum ErrorBody {
    /// Parsed JSON object or array.
    Json(serde_json::Value),
    /// Non-JSON text.
    Text(String),
    /// Empty body.
    Empty,
}

pub(crate) fn parse_error_body(bytes: &[u8]) -> ErrorBody {
    if bytes.is_empty() {
        return ErrorBody::Empty;
    }
    match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(value) => ErrorBody::Json(value),
        Err(_) => ErrorBody::Text(String::from_utf8_lossy(bytes).into_owned()),
    }
}

fn extract_message(body: &serde_json::Value) -> Option<String> {
    let obj = body.as_object()?;
    if let Some(s) = obj.get("error").and_then(serde_json::Value::as_str) {
        return Some(s.to_owned());
    }
    if let Some(s) = obj
        .get("error")
        .and_then(|v| v.get("message"))
        .and_then(serde_json::Value::as_str)
    {
        return Some(s.to_owned());
    }
    if let Some(s) = obj.get("message").and_then(serde_json::Value::as_str) {
        return Some(s.to_owned());
    }
    if let Some(s) = obj.get("detail").and_then(serde_json::Value::as_str) {
        return Some(s.to_owned());
    }
    None
}

pub(crate) fn redact_secrets(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    const NEEDLE: &str = "Bearer ";
    while let Some(i) = rest.find(NEEDLE) {
        out.push_str(&rest[..i + NEEDLE.len()]);
        rest = &rest[i + NEEDLE.len()..];
        let skip = rest
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(rest.len());
        if skip > 0 {
            out.push_str("[redacted]");
            rest = &rest[skip..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_key_mentions_env_var() {
        let msg = Error::MissingApiKey.to_string();
        assert!(msg.contains("TYPESAFE_API_KEY"));
        assert!(!msg.contains("sk-"));
    }

    #[test]
    fn display_redacts_bearer_tokens() {
        let te = TransportError {
            message: redact_secrets("Connection error: Bearer sk-secret-value-1234"),
            pre_send: true,
        };
        let rendered = Error::Connection(te).to_string();
        assert!(!rendered.contains("sk-secret"));
        assert!(rendered.contains("[redacted]"));
    }

    #[test]
    fn api_display_omits_raw_body() {
        let err = ApiError::from_response(
            StatusCode::BAD_REQUEST,
            ErrorBody::Json(serde_json::json!({
                "error": "bad",
                "request": { "api_key": "sk-should-not-appear-in-full-dump" }
            })),
            HeaderMap::new(),
            "/v1/systemone",
            1,
        );
        let rendered = err.to_string();
        assert!(rendered.contains("400"));
        assert!(rendered.contains("bad"));
        assert!(!rendered.contains("sk-should-not-appear-in-full-dump"));
    }

    #[test]
    fn classifies_status_codes() {
        assert_eq!(
            ApiErrorKind::from_status(StatusCode::TOO_MANY_REQUESTS),
            ApiErrorKind::RateLimit
        );
        assert_eq!(
            ApiErrorKind::from_status(StatusCode::from_u16(529).unwrap()),
            ApiErrorKind::InternalServer
        );
        assert_eq!(
            ApiErrorKind::from_status(StatusCode::CONFLICT),
            ApiErrorKind::Conflict
        );
    }

    #[test]
    fn helpers_classify_api_and_timeout() {
        let err = Error::from(ApiError::from_response(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorBody::Empty,
            HeaderMap::new(),
            "/v1/systemone",
            2,
        ));
        assert!(err.is_rate_limited());
        assert!(!err.is_auth());
        assert_eq!(err.kind(), Some(ApiErrorKind::RateLimit));
        assert_eq!(err.attempts(), Some(2));

        let timeout = Error::Timeout {
            after: Duration::from_secs(10),
        };
        assert!(timeout.is_timeout());
        assert!(!timeout.is_connection());
        assert!(timeout.as_api().is_none());
    }
}
