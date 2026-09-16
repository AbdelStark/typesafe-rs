use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use http::{HeaderMap, StatusCode};

use crate::error::Error;

/// Set of HTTP status codes that should be retried.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusSet {
    codes: HashSet<u16>,
}

impl StatusSet {
    /// Empty set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            codes: HashSet::new(),
        }
    }

    /// Official default: 408, 429, and every 5xx.
    #[must_use]
    pub fn retryable() -> Self {
        let mut codes = HashSet::with_capacity(102);
        codes.insert(408);
        codes.insert(429);
        for status in 500..=599 {
            codes.insert(status);
        }
        Self { codes }
    }

    /// Build from explicit codes.
    #[must_use]
    pub fn from_codes(codes: impl IntoIterator<Item = u16>) -> Self {
        Self {
            codes: codes.into_iter().collect(),
        }
    }

    /// Insert a status code.
    pub fn insert(&mut self, code: u16) {
        self.codes.insert(code);
    }

    /// Whether `code` is in the set.
    #[must_use]
    pub fn contains(&self, code: u16) -> bool {
        self.codes.contains(&code)
    }

    /// Whether `status` is in the set.
    #[must_use]
    pub fn contains_status(&self, status: StatusCode) -> bool {
        self.contains(status.as_u16())
    }
}

impl Default for StatusSet {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<u16> for StatusSet {
    fn from_iter<T: IntoIterator<Item = u16>>(iter: T) -> Self {
        Self::from_codes(iter)
    }
}

/// Retry policy matching the official TypeSafe SDKs.
///
/// Default: 2 retries, 500 ms initial backoff, 5 s cap, 25% jitter, HTTP 408/429/5xx,
/// connection errors, timeouts, and `retry-after-ms` / `Retry-After` up to 60 s.
///
/// Struct-update from [`RetryPolicy::default`] so private fields stay initialized:
/// `RetryPolicy { max_retries: 0, ..RetryPolicy::default() }`.
///
/// # Examples
///
/// ```
/// use typesafe_rs::RetryPolicy;
///
/// let none = RetryPolicy::none();
/// assert_eq!(none.max_retries, 0);
///
/// let conservative = RetryPolicy::conservative();
/// assert!(!conservative.retry_timeouts);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct RetryPolicy {
    /// Maximum retries after the initial attempt. Default: 2.
    pub max_retries: u32,
    /// First backoff delay. Default: 500 ms.
    pub backoff_initial: Duration,
    /// Backoff cap. Default: 5 s.
    pub backoff_max: Duration,
    /// Fraction of each backoff randomly subtracted, in `[0, 1]`. Default: 0.25.
    pub backoff_jitter: f64,
    /// HTTP statuses that are retried. Default: 408, 429, 500–599.
    pub http_statuses: StatusSet,
    /// Honor `retry-after-ms` / `Retry-After` up to [`Self::max_retry_after`].
    pub respect_retry_after: bool,
    /// Maximum server-requested delay that will be honored. Default: 60 s.
    pub max_retry_after: Duration,
    /// Retry connection failures. Default: true.
    pub retry_connection_errors: bool,
    /// Retry timeouts. Default: true.
    pub retry_timeouts: bool,
    /// When true, only connection errors that happen before bytes are sent are retried.
    ///
    /// Set by [`RetryPolicy::conservative`]. Keep the default (`false`) unless you
    /// want that behaviour; prefer `..RetryPolicy::default()` in struct updates.
    pub connect_only_before_send: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            backoff_initial: Duration::from_millis(500),
            backoff_max: Duration::from_secs(5),
            backoff_jitter: 0.25,
            http_statuses: StatusSet::retryable(),
            respect_retry_after: true,
            max_retry_after: Duration::from_secs(60),
            retry_connection_errors: true,
            retry_timeouts: true,
            connect_only_before_send: false,
        }
    }
}

impl RetryPolicy {
    /// No retries (`max_retries = 0`).
    #[must_use]
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Retry 408/429 and pre-send connection errors only. No 5xx, no timeouts.
    #[must_use]
    pub fn conservative() -> Self {
        Self {
            http_statuses: StatusSet::from_codes([408, 429]),
            retry_timeouts: false,
            retry_connection_errors: true,
            connect_only_before_send: true,
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        if !(0.0..=1.0).contains(&self.backoff_jitter) {
            return Err(Error::InvalidRequest(format!(
                "`retry.backoff_jitter` must be between 0 and 1, got {}.",
                self.backoff_jitter
            )));
        }
        Ok(())
    }

    pub(crate) fn retries_connection(&self, pre_send: bool) -> bool {
        if !self.retry_connection_errors {
            return false;
        }
        if self.connect_only_before_send {
            return pre_send;
        }
        true
    }

    /// Parse `retry-after-ms` (preferred) or `Retry-After` into a delay.
    ///
    /// Invalid values are ignored (`None`).
    #[must_use]
    pub fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
        Self::parse_retry_after_at(headers, SystemTime::now())
    }

    /// [`Self::parse_retry_after`] with an injected `now` for HTTP-date values.
    #[must_use]
    pub fn parse_retry_after_at(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
        if let Some(value) = headers.get("retry-after-ms") {
            if let Ok(raw) = value.to_str() {
                if let Ok(ms) = raw.trim().parse::<f64>() {
                    if ms.is_finite() && ms >= 0.0 {
                        return Some(duration_from_millis_f64(ms));
                    }
                }
            }
        }

        let raw = headers.get("retry-after")?.to_str().ok()?.trim();
        if let Ok(seconds) = raw.parse::<f64>() {
            if seconds.is_finite() && seconds >= 0.0 {
                return Some(duration_from_millis_f64(seconds * 1000.0));
            }
            return None;
        }
        let date = httpdate::parse_http_date(raw).ok()?;
        let delay = date.duration_since(now).unwrap_or_default();
        Some(delay)
    }

    /// Delay before the next attempt after a zero-based failed `attempt`.
    #[must_use]
    pub fn delay_after_failure(&self, attempt: u32, headers: Option<&HeaderMap>) -> Duration {
        self.delay_after_failure_with(attempt, headers, fastrand::f64)
    }

    /// [`Self::delay_after_failure`] with an injected RNG in `[0, 1)`.
    ///
    /// Tests should call this instead of reimplementing backoff math.
    pub fn delay_after_failure_with<F>(
        &self,
        attempt: u32,
        headers: Option<&HeaderMap>,
        rng: F,
    ) -> Duration
    where
        F: FnOnce() -> f64,
    {
        if self.respect_retry_after {
            if let Some(headers) = headers {
                if let Some(delay) = Self::parse_retry_after(headers) {
                    if delay <= self.max_retry_after {
                        return delay;
                    }
                }
            }
        }

        let factor = 1u32.checked_shl(attempt).unwrap_or(u32::MAX);
        let exponential = self
            .backoff_initial
            .saturating_mul(factor)
            .min(self.backoff_max);
        let exp_ms = exponential.as_secs_f64() * 1000.0;
        let jitter = rng().clamp(0.0, 1.0) * self.backoff_jitter;
        duration_from_millis_f64((exp_ms * (1.0 - jitter)).round().max(0.0))
    }
}

fn duration_from_millis_f64(ms: f64) -> Duration {
    let nanos = (ms * 1_000_000.0).round().max(0.0) as u128;
    let nanos = u64::try_from(nanos).unwrap_or(u64::MAX);
    Duration::from_nanos(nanos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    fn policy() -> RetryPolicy {
        RetryPolicy::default()
    }

    #[test]
    fn default_statuses_include_408_429_and_5xx() {
        let s = StatusSet::retryable();
        assert!(s.contains(408));
        assert!(s.contains(429));
        assert!(s.contains(500));
        assert!(s.contains(529));
        assert!(s.contains(599));
        assert!(!s.contains(400));
        assert!(!s.contains(422));
    }

    #[test]
    fn delay_without_headers_zero_jitter_is_initial() {
        let d = policy().delay_after_failure_with(0, None, || 0.0);
        assert_eq!(d, Duration::from_millis(500));
    }

    #[test]
    fn delay_full_jitter_subtracts_25_percent() {
        let d = policy().delay_after_failure_with(0, None, || 1.0);
        assert_eq!(d, Duration::from_millis(375));
    }

    #[test]
    fn delay_doubles_per_attempt_until_cap() {
        let p = policy();
        assert_eq!(
            p.delay_after_failure_with(1, None, || 0.0),
            Duration::from_millis(1000)
        );
        assert_eq!(
            p.delay_after_failure_with(2, None, || 0.0),
            Duration::from_millis(2000)
        );
        assert_eq!(
            p.delay_after_failure_with(4, None, || 0.0),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn retry_after_ms_wins_over_backoff() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after-ms", HeaderValue::from_static("120"));
        let d = policy().delay_after_failure_with(0, Some(&headers), || 0.0);
        assert_eq!(d, Duration::from_millis(120));
    }

    #[test]
    fn retry_after_ms_over_max_falls_back_to_backoff() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after-ms", HeaderValue::from_static("70000"));
        let d = policy().delay_after_failure_with(0, Some(&headers), || 0.0);
        assert_eq!(d, Duration::from_millis(500));
    }

    #[test]
    fn retry_after_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("2"));
        let d = RetryPolicy::parse_retry_after(&headers).unwrap();
        assert_eq!(d, Duration::from_secs(2));
    }

    #[test]
    fn retry_after_http_date() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let later = now + Duration::from_secs(4);
        let mut headers = HeaderMap::new();
        headers.insert(
            "retry-after",
            HeaderValue::from_str(&httpdate::fmt_http_date(later)).unwrap(),
        );
        let d = RetryPolicy::parse_retry_after_at(&headers, now).unwrap();
        assert_eq!(d, Duration::from_secs(4));
    }

    #[test]
    fn past_http_date_clamps_to_zero() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let past = SystemTime::UNIX_EPOCH;
        let mut headers = HeaderMap::new();
        headers.insert(
            "retry-after",
            HeaderValue::from_str(&httpdate::fmt_http_date(past)).unwrap(),
        );
        let d = RetryPolicy::parse_retry_after_at(&headers, now).unwrap();
        assert_eq!(d, Duration::ZERO);
    }

    #[test]
    fn invalid_retry_after_ignored() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after-ms", HeaderValue::from_static("nope"));
        headers.insert("retry-after", HeaderValue::from_static("also-nope"));
        assert!(RetryPolicy::parse_retry_after(&headers).is_none());
    }

    #[test]
    fn negative_retry_after_ignored() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("-1"));
        assert!(RetryPolicy::parse_retry_after(&headers).is_none());
    }

    #[test]
    fn retry_after_ms_preferred_over_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after-ms", HeaderValue::from_static("50"));
        headers.insert("retry-after", HeaderValue::from_static("9"));
        let d = RetryPolicy::parse_retry_after(&headers).unwrap();
        assert_eq!(d, Duration::from_millis(50));
    }

    #[test]
    fn none_has_zero_retries() {
        assert_eq!(RetryPolicy::none().max_retries, 0);
    }

    #[test]
    fn conservative_skips_5xx_and_timeouts() {
        let p = RetryPolicy::conservative();
        assert!(!p.http_statuses.contains(500));
        assert!(p.http_statuses.contains(429));
        assert!(p.http_statuses.contains(408));
        assert!(!p.retry_timeouts);
        assert!(p.retry_connection_errors);
        assert!(p.connect_only_before_send);
        assert!(p.retries_connection(true));
        assert!(!p.retries_connection(false));
    }

    #[test]
    fn default_retries_all_connection_errors() {
        let p = RetryPolicy::default();
        assert!(p.retries_connection(true));
        assert!(p.retries_connection(false));
    }
}
