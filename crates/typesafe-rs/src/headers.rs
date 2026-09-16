use http::{HeaderMap, HeaderName, HeaderValue};

use crate::VERSION;

const PROTECTED: &[&str] = &[
    "authorization",
    "accept",
    "user-agent",
    "x-typesafe-sdk",
    "x-typesafe-runtime",
    "x-typesafe-retry-count",
];

pub(crate) fn is_protected(name: &HeaderName) -> bool {
    PROTECTED.contains(&name.as_str())
}

/// Copy user headers, ignoring names the SDK owns.
pub(crate) fn merge_user_headers(dst: &mut HeaderMap, src: &HeaderMap) {
    for (name, value) in src {
        if !is_protected(name) {
            dst.insert(name.clone(), value.clone());
        }
    }
}

pub(crate) fn user_agent() -> HeaderValue {
    static_or_owned(&format!("typesafe-rs/{VERSION}"))
}

pub(crate) fn sdk_header() -> HeaderValue {
    static_or_owned(&format!("typesafe-rs/{VERSION}"))
}

pub(crate) fn runtime_header() -> HeaderValue {
    static_or_owned(&format!(
        "rust/{}; {}-{}",
        env!("TYPESAFE_RUSTC_VERSION"),
        env!("TYPESAFE_TARGET_OS"),
        env!("TYPESAFE_TARGET_ARCH"),
    ))
}

fn static_or_owned(value: &str) -> HeaderValue {
    HeaderValue::from_str(value).unwrap_or_else(|_| HeaderValue::from_static("typesafe-rs"))
}

pub(crate) fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-typesafe-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

pub(crate) fn retry_count_value(attempt: u32) -> HeaderValue {
    HeaderValue::from_str(&attempt.to_string()).unwrap_or_else(|_| HeaderValue::from_static("1"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::{ACCEPT, AUTHORIZATION, USER_AGENT};

    #[test]
    fn protected_headers_are_skipped() {
        let mut dst = HeaderMap::new();
        let mut src = HeaderMap::new();
        src.insert(AUTHORIZATION, HeaderValue::from_static("Bearer stolen"));
        src.insert(ACCEPT, HeaderValue::from_static("text/plain"));
        src.insert(USER_AGENT, HeaderValue::from_static("other"));
        src.insert("x-typesafe-sdk", HeaderValue::from_static("nope"));
        src.insert("x-custom", HeaderValue::from_static("ok"));
        merge_user_headers(&mut dst, &src);
        assert!(dst.get(AUTHORIZATION).is_none());
        assert!(dst.get(ACCEPT).is_none());
        assert_eq!(dst.get("x-custom").unwrap(), "ok");
    }
}
