use std::fmt;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue};
use url::Url;

use crate::error::Error;
use crate::headers::is_protected;
use crate::retry::RetryPolicy;

/// Environment variable for the API key.
pub const ENV_API_KEY: &str = "TYPESAFE_API_KEY";
/// Environment variable for the API root URL.
pub const ENV_BASE_URL: &str = "TYPESAFE_BASE_URL";
/// Environment variable for the default model.
pub const ENV_DEFAULT_MODEL: &str = "TYPESAFE_DEFAULT_MODEL";

/// Default API root.
pub const DEFAULT_BASE_URL: &str = "https://api.typesafe.ai";
/// Default model when none is configured.
pub const DEFAULT_MODEL: &str = "jev-latest";
/// Default per-attempt timeout, including the response body.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// API key wrapper that redacts itself in [`Debug`].
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    /// Wrap an owned secret.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([redacted])")
    }
}

/// Client configuration. Explicit values win over environment, then defaults.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// API key. Falls back to [`ENV_API_KEY`].
    pub api_key: Option<SecretString>,
    /// API root. Falls back to [`ENV_BASE_URL`], then [`DEFAULT_BASE_URL`].
    pub base_url: Option<Url>,
    /// Default model. Falls back to [`ENV_DEFAULT_MODEL`], then [`DEFAULT_MODEL`].
    pub default_model: Option<String>,
    /// Per-attempt timeout including the body. Default: 10 s.
    pub timeout: Duration,
    /// Retry policy. Default matches the official SDKs.
    pub retry: RetryPolicy,
    /// Extra headers. Cannot override Authorization, Accept, or SDK identification headers.
    pub default_headers: HeaderMap,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            default_model: None,
            timeout: DEFAULT_TIMEOUT,
            retry: RetryPolicy::default(),
            default_headers: HeaderMap::new(),
        }
    }
}

impl ClientConfig {
    /// Empty config that will read the process environment in [`crate::Client::new`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the API key.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<SecretString>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the API root.
    #[must_use]
    pub fn base_url(mut self, url: Url) -> Self {
        self.base_url = Some(url);
        self
    }

    /// Parse and set the API root.
    pub fn try_base_url(mut self, url: &str) -> Result<Self, Error> {
        self.base_url = Some(parse_base_url(url)?);
        Ok(self)
    }

    /// Set the default model.
    #[must_use]
    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Set the per-attempt timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the retry policy.
    #[must_use]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// Insert a default header. Protected names (`Authorization`, `Accept`,
    /// `User-Agent`, SDK identification, retry-count) are ignored.
    pub fn header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Result<Self, Error> {
        insert_user_header(&mut self.default_headers, name.as_ref(), value.as_ref())?;
        Ok(self)
    }

    /// Replace extra default headers. Protected names are ignored at send time.
    #[must_use]
    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        self.default_headers = headers;
        self
    }

    /// Build an async [`Client`](crate::Client).
    ///
    /// # Errors
    ///
    /// Same as [`crate::Client::new`].
    pub fn build(self) -> Result<crate::Client, Error> {
        crate::Client::new(self)
    }

    /// Build a [`BlockingClient`](crate::BlockingClient).
    ///
    /// # Errors
    ///
    /// Same as [`crate::BlockingClient::new`].
    #[cfg(feature = "blocking")]
    #[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
    pub fn build_blocking(self) -> Result<crate::BlockingClient, Error> {
        crate::BlockingClient::new(self)
    }

    /// Fill unset fields from `lookup` (typically the process environment).
    ///
    /// Empty or whitespace-only values are ignored. Explicit fields are left unchanged.
    pub fn overlay_env(
        &mut self,
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<(), Error> {
        if self.api_key.is_none() {
            if let Some(value) = lookup(ENV_API_KEY).and_then(trim_nonempty) {
                self.api_key = Some(SecretString::from(value));
            }
        }
        if self.base_url.is_none() {
            if let Some(value) = lookup(ENV_BASE_URL).and_then(trim_nonempty) {
                self.base_url = Some(parse_base_url(&value)?);
            }
        }
        if self.default_model.is_none() {
            if let Some(value) = lookup(ENV_DEFAULT_MODEL).and_then(trim_nonempty) {
                self.default_model = Some(value);
            }
        }
        Ok(())
    }
}

/// Per-call overrides.
#[derive(Clone, Debug, Default)]
pub struct CallOptions {
    /// Override the per-attempt timeout.
    pub timeout: Option<Duration>,
    /// Override the retry policy.
    pub retry: Option<RetryPolicy>,
    /// Extra headers for this call (protected names are ignored).
    pub headers: HeaderMap,
    /// Override the model for `system_one`.
    pub model: Option<String>,
}

impl CallOptions {
    /// No overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Override timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Override retry policy.
    #[must_use]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Override model.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Insert a per-call header. Protected names are ignored.
    pub fn header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Result<Self, Error> {
        insert_user_header(&mut self.headers, name.as_ref(), value.as_ref())?;
        Ok(self)
    }

    /// Replace extra per-call headers. Protected names are ignored at send time.
    #[must_use]
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }
}

fn insert_user_header(map: &mut HeaderMap, name: &str, value: &str) -> Result<(), Error> {
    let name = HeaderName::from_bytes(name.as_bytes())
        .map_err(|err| Error::InvalidRequest(format!("invalid header name {name:?}: {err}")))?;
    let value = HeaderValue::from_str(value)
        .map_err(|err| Error::InvalidRequest(format!("invalid header value: {err}")))?;
    if !is_protected(&name) {
        map.insert(name, value);
    }
    Ok(())
}

pub(crate) fn trim_nonempty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub(crate) fn parse_base_url(raw: &str) -> Result<Url, Error> {
    Url::parse(raw).map_err(|err| Error::InvalidRequest(format!("invalid base URL {raw:?}: {err}")))
}

pub(crate) fn strip_trailing_slashes(url: Url) -> Url {
    let stripped = url.as_str().trim_end_matches('/');
    Url::parse(stripped).unwrap_or(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn overlay_code_wins_over_env() {
        let mut cfg = ClientConfig::default()
            .api_key("from-code")
            .default_model("code-model");
        let env = HashMap::from([(ENV_API_KEY, "from-env"), (ENV_DEFAULT_MODEL, "env-model")]);
        cfg.overlay_env(|k| env.get(k).map(|s| (*s).to_owned()))
            .unwrap();
        assert_eq!(cfg.api_key.as_ref().unwrap().expose(), "from-code");
        assert_eq!(cfg.default_model.as_deref(), Some("code-model"));
    }

    #[test]
    fn overlay_env_fills_unset() {
        let mut cfg = ClientConfig::default().api_key("from-code");
        let env = HashMap::from([(ENV_DEFAULT_MODEL, "env-model")]);
        cfg.overlay_env(|k| env.get(k).map(|s| (*s).to_owned()))
            .unwrap();
        assert_eq!(cfg.api_key.as_ref().unwrap().expose(), "from-code");
        assert_eq!(cfg.default_model.as_deref(), Some("env-model"));
    }

    #[test]
    fn overlay_ignores_blank_env() {
        let mut cfg = ClientConfig::default();
        cfg.overlay_env(|k| match k {
            ENV_API_KEY => Some("   ".into()),
            ENV_DEFAULT_MODEL => Some(String::new()),
            _ => None,
        })
        .unwrap();
        assert!(cfg.api_key.is_none());
        assert!(cfg.default_model.is_none());
    }

    #[test]
    fn secret_debug_redacts() {
        let s = SecretString::from("sk-live-super-secret");
        assert!(!format!("{s:?}").contains("sk-live"));
        assert!(format!("{s:?}").contains("redacted"));
    }

    #[test]
    fn invalid_env_base_url_errors() {
        let mut cfg = ClientConfig::default();
        let err = cfg
            .overlay_env(|k| {
                if k == ENV_BASE_URL {
                    Some("not a url".into())
                } else {
                    None
                }
            })
            .unwrap_err();
        assert!(matches!(err, Error::InvalidRequest(_)));
    }

    #[test]
    fn strip_trailing_slashes_keeps_path_prefix() {
        let url = Url::parse("https://example.com/prefix/").unwrap();
        let stripped = strip_trailing_slashes(url);
        assert_eq!(stripped.as_str(), "https://example.com/prefix");
    }

    #[test]
    fn header_builder_skips_protected_names() {
        let cfg = ClientConfig::new()
            .header("x-custom", "ok")
            .unwrap()
            .header("authorization", "Bearer stolen")
            .unwrap();
        assert_eq!(
            cfg.default_headers
                .get("x-custom")
                .and_then(|v| v.to_str().ok()),
            Some("ok")
        );
        assert!(cfg.default_headers.get("authorization").is_none());
    }

    #[test]
    fn invalid_header_name_errors() {
        let err = ClientConfig::new().header("not a name", "v").unwrap_err();
        assert!(matches!(err, Error::InvalidRequest(_)));
    }
}
