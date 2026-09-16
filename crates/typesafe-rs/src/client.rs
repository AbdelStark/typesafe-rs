use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Serialize;
use url::Url;

use crate::config::{
    CallOptions, ClientConfig, DEFAULT_BASE_URL, DEFAULT_MODEL, strip_trailing_slashes,
};
use crate::error::{ApiError, Error, parse_error_body};
use crate::headers::{
    merge_user_headers, request_id, retry_count_value, runtime_header, sdk_header, user_agent,
};
use crate::retry::RetryPolicy;
use crate::transport::{HttpTransport, RawResponse, join_endpoint};
use crate::types::{
    ListModelsResponse, ModelCard, Questions, ResponseMeta, SystemOneRequest, SystemOneResponse,
    validate_questions,
};

#[derive(Debug)]
struct Inner {
    http: HttpTransport,
    api_key: crate::config::SecretString,
    base_url: Url,
    default_model: String,
    timeout: Duration,
    retry: RetryPolicy,
    default_headers: HeaderMap,
}

/// Asynchronous TypeSafe System One client.
///
/// Cheap to clone (`Arc` internally) and safe to share across tasks (`Send + Sync`).
///
/// # Examples
///
/// ```no_run
/// use typesafe_rs::{questions, Client, ClientConfig, Question};
///
/// # async fn run() -> typesafe_rs::Result<()> {
/// let client = ClientConfig::new().api_key("sk-...").build()?;
/// let response = client
///     .system_one(
///         "Help! My payouts have been failing for 3 days.",
///         questions! { "urgent" => Question::noul("Does this convey urgency?") },
///     )
///     .await?;
/// assert!(response.noul("urgent").is_some());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Client {
    inner: Arc<Inner>,
}

impl Client {
    /// Build a client. Unset fields fall back to the process environment, then defaults.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingApiKey`] when no key is configured, or
    /// [`Error::InvalidRequest`] for invalid timeout / retry / URL values.
    pub fn new(config: ClientConfig) -> Result<Self, Error> {
        Self::new_with_env(config, |key| std::env::var(key).ok())
    }

    /// [`Self::new`] using `TYPESAFE_*` from the process environment.
    pub fn from_env() -> Result<Self, Error> {
        Self::new(ClientConfig::default())
    }

    /// Like [`Self::new`], but environment fallbacks come from `lookup`.
    ///
    /// Intended for tests and embeddings that should not read process env.
    pub fn new_with_env(
        mut config: ClientConfig,
        lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, Error> {
        config.overlay_env(lookup)?;
        if config.timeout.is_zero() {
            return Err(Error::InvalidRequest(
                "`timeout` must be a positive duration".to_owned(),
            ));
        }
        config.retry.validate()?;
        let api_key = config.api_key.ok_or(Error::MissingApiKey)?;
        let base_url =
            strip_trailing_slashes(config.base_url.unwrap_or_else(|| {
                Url::parse(DEFAULT_BASE_URL).expect("default base URL is valid")
            }));
        let default_model = config
            .default_model
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_owned());
        Ok(Self {
            inner: Arc::new(Inner {
                http: HttpTransport::new()?,
                api_key,
                base_url,
                default_model,
                timeout: config.timeout,
                retry: config.retry,
                default_headers: config.default_headers,
            }),
        })
    }

    /// Default model used when a request omits `model`.
    #[must_use]
    pub fn default_model(&self) -> &str {
        &self.inner.default_model
    }

    /// Resolved API root, without a trailing slash.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.inner.base_url
    }

    /// Evaluate `state` against `questions` using client defaults.
    ///
    /// `state` may be a string, object, or array. Question keys are returned on
    /// [`SystemOneResponse::answers`](crate::SystemOneResponse::answers).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidRequest`] before any network call when the
    /// question map is empty or a Choice/Score is under-specified. Network and
    /// API failures use the rest of [`Error`].
    pub async fn system_one(
        &self,
        state: impl Serialize,
        questions: Questions,
    ) -> Result<SystemOneResponse, Error> {
        let state = serde_json::to_value(state).map_err(|err| {
            Error::InvalidRequest(format!("state is not JSON-serializable: {err}"))
        })?;
        let req = SystemOneRequest::new(state, questions);
        self.system_one_with(&req, CallOptions::default()).await
    }

    /// Evaluate a fully specified request with per-call options.
    pub async fn system_one_with(
        &self,
        req: &SystemOneRequest,
        opts: CallOptions,
    ) -> Result<SystemOneResponse, Error> {
        validate_questions(&req.questions)?;
        let model = opts
            .model
            .as_deref()
            .filter(|m| !m.is_empty())
            .or_else(|| {
                if req.model.trim().is_empty() {
                    None
                } else {
                    Some(req.model.as_str())
                }
            })
            .unwrap_or(self.inner.default_model.as_str())
            .to_owned();
        let payload = serde_json::json!({
            "state": req.state,
            "model": model,
            "questions": req.questions,
        });
        let body =
            Bytes::from(serde_json::to_vec(&payload).map_err(|err| {
                Error::InvalidRequest(format!("failed to serialize request: {err}"))
            })?);

        let raw = {
            let fut = self.execute(Method::POST, "/v1/systemone", Some(body), &opts);
            #[cfg(feature = "tracing")]
            {
                use tracing::Instrument;
                let span = tracing::info_span!(
                    "typesafe.request",
                    http.request.method = "POST",
                    url.path = "/v1/systemone",
                    typesafe.model = model.as_str(),
                    typesafe.questions.count = req.questions.len(),
                );
                fut.instrument(span).await?
            }
            #[cfg(not(feature = "tracing"))]
            {
                fut.await?
            }
        };
        let mut parsed: SystemOneResponse = decode_json(&raw, "/v1/systemone")?;
        parsed.meta = meta_from_raw(&raw);
        Ok(parsed)
    }

    /// Access the Models resource.
    #[must_use]
    pub fn models(&self) -> Models<'_> {
        Models { client: self }
    }

    /// Establish a pooled connection by calling `GET /v1/models`.
    pub async fn warm_up(&self) -> Result<(), Error> {
        let raw = self
            .execute(Method::GET, "/v1/models", None, &CallOptions::default())
            .await?;
        let _ = raw;
        Ok(())
    }

    async fn execute(
        &self,
        method: Method,
        path: &'static str,
        body: Option<Bytes>,
        opts: &CallOptions,
    ) -> Result<RawResponse, Error> {
        let timeout = opts.timeout.unwrap_or(self.inner.timeout);
        if timeout.is_zero() {
            return Err(Error::InvalidRequest(
                "`timeout` must be a positive duration".to_owned(),
            ));
        }
        let retry = opts
            .retry
            .clone()
            .unwrap_or_else(|| self.inner.retry.clone());
        retry.validate()?;

        let url = join_endpoint(&self.inner.base_url, path)?;
        let mut attempt = 0_u32;
        loop {
            let retries_left = retry.max_retries.saturating_sub(attempt);
            let headers = self.attempt_headers(opts, body.is_some(), attempt);
            match self
                .inner
                .http
                .send(method.clone(), url.clone(), headers, body.clone(), timeout)
                .await
            {
                Ok(mut raw) if raw.status.is_success() => {
                    raw.attempts = attempt + 1;
                    return Ok(raw);
                }
                Ok(raw) => {
                    let retryable = retry.http_statuses.contains_status(raw.status);
                    if retries_left > 0 && retryable {
                        let delay = retry.delay_after_failure(attempt, Some(&raw.headers));
                        emit_retry(attempt, delay, &raw.status.to_string());
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    let body = parse_error_body(&raw.body);
                    return Err(Error::Api(Box::new(ApiError::from_response(
                        raw.status,
                        body,
                        raw.headers,
                        path,
                        attempt + 1,
                    ))));
                }
                Err(err) => {
                    let retryable = match &err {
                        Error::Timeout { .. } => retry.retry_timeouts,
                        Error::Connection(te) => retry.retries_connection(te.is_pre_send()),
                        _ => false,
                    };
                    if retries_left > 0 && retryable {
                        let delay = retry.delay_after_failure(attempt, None);
                        emit_retry(attempt, delay, &err.to_string());
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(err);
                }
            }
        }
    }

    fn attempt_headers(&self, opts: &CallOptions, has_body: bool, attempt: u32) -> HeaderMap {
        let mut headers = HeaderMap::new();
        merge_user_headers(&mut headers, &self.inner.default_headers);
        merge_user_headers(&mut headers, &opts.headers);
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.inner.api_key.expose()))
                .unwrap_or_else(|_| HeaderValue::from_static("Bearer")),
        );
        headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(header::USER_AGENT, user_agent());
        headers.insert("x-typesafe-sdk", sdk_header());
        headers.insert("x-typesafe-runtime", runtime_header());
        if has_body {
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
        }
        if attempt > 0 {
            headers.insert("x-typesafe-retry-count", retry_count_value(attempt));
        } else {
            headers.remove("x-typesafe-retry-count");
        }
        headers
    }
}

/// Models API resource.
#[derive(Clone, Copy, Debug)]
pub struct Models<'a> {
    client: &'a Client,
}

impl Models<'_> {
    /// List models available to the account.
    ///
    /// A body without a `models` array returns [`Error::UnexpectedShape`].
    pub async fn list(&self) -> Result<Vec<ModelCard>, Error> {
        self.list_with(&CallOptions::default()).await
    }

    /// [`Self::list`] with per-call options.
    pub async fn list_with(&self, opts: &CallOptions) -> Result<Vec<ModelCard>, Error> {
        let raw = self
            .client
            .execute(Method::GET, "/v1/models", None, opts)
            .await?;
        let value: serde_json::Value = decode_json(&raw, "/v1/models")?;
        match value.get("models") {
            Some(serde_json::Value::Array(_)) => {
                let parsed: ListModelsResponse =
                    serde_json::from_value(value).map_err(|source| Error::Decode {
                        source,
                        body: raw.body.clone(),
                        meta: Box::new(meta_from_raw(&raw)),
                    })?;
                Ok(parsed.models)
            }
            _ => Err(Error::UnexpectedShape {
                endpoint: "GET /v1/models",
                meta: Box::new(meta_from_raw(&raw)),
            }),
        }
    }
}

fn decode_json<T: serde::de::DeserializeOwned>(
    raw: &RawResponse,
    endpoint: &'static str,
) -> Result<T, Error> {
    serde_json::from_slice(&raw.body).map_err(|source| {
        let _ = endpoint;
        Error::Decode {
            source,
            body: raw.body.clone(),
            meta: Box::new(meta_from_raw(raw)),
        }
    })
}

fn meta_from_raw(raw: &RawResponse) -> ResponseMeta {
    ResponseMeta {
        request_id: request_id(&raw.headers),
        status: Some(raw.status),
        headers: raw.headers.clone(),
        attempts: raw.attempts,
    }
}

fn emit_retry(attempt: u32, delay: Duration, reason: &str) {
    let _ = (attempt, delay, reason);
    #[cfg(feature = "tracing")]
    {
        tracing::info!(
            attempt,
            delay_ms = delay.as_millis() as u64,
            reason,
            "retry_scheduled"
        );
    }
}
