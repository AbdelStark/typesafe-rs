//! In-process HTTP mock for [`typesafe-rs`](https://docs.rs/typesafe-rs).
//!
//! Binds `127.0.0.1:0`, scripts `POST /v1/systemone` and `GET /v1/models`, and
//! records a request journal. Tests use a real `Client` over loopback.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{HeaderMap as AxumHeaderMap, HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::{Value, json};
use tokio::sync::oneshot;
use url::Url;

/// Running mock API, bound to a random localhost port.
pub struct MockServer {
    url: Url,
    inner: Arc<Mutex<Inner>>,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

struct Inner {
    stubs: Vec<Stub>,
    journal: Vec<RecordedRequest>,
    request_counter: u64,
}

#[derive(Clone)]
struct Stub {
    endpoint: Endpoint,
    matcher: Matcher,
    status: u16,
    headers: Vec<(String, String)>,
    body: Value,
    remaining: Option<u32>,
    delay: Option<Duration>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Endpoint {
    SystemOne,
    Models,
}

#[derive(Clone)]
enum Matcher {
    Any,
    QuestionKey(String),
}

/// One observed HTTP request.
#[derive(Clone, Debug)]
pub struct RecordedRequest {
    /// HTTP method, uppercase.
    pub method: String,
    /// Request path, e.g. `/v1/systemone`.
    pub path: String,
    /// Headers keyed by lowercase name; last value wins.
    pub headers: HashMap<String, String>,
    /// Parsed JSON body, when present and valid.
    pub body: Option<Value>,
    /// Local time the mock accepted the request.
    pub received_at: Instant,
}

impl MockServer {
    /// Bind `127.0.0.1:0` and serve until dropped.
    pub async fn start() -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            stubs: Vec::new(),
            journal: Vec::new(),
            request_counter: 0,
        }));
        let app = Router::new()
            .route("/v1/systemone", post(system_one))
            .route("/v1/models", get(models))
            .fallback(fallback)
            .with_state(inner.clone());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock listener");
        let addr = listener.local_addr().expect("local_addr");
        let url = Url::parse(&format!("http://{addr}")).expect("url");
        let (tx, rx) = oneshot::channel();
        let join = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
        });

        Self {
            url,
            inner,
            shutdown: Some(tx),
            join: Some(join),
        }
    }

    /// Base URL with no trailing slash, e.g. `http://127.0.0.1:12345`.
    #[must_use]
    pub fn url(&self) -> Url {
        self.url.clone()
    }

    /// Script `POST /v1/systemone` responses. The stub is registered when the builder is dropped.
    pub fn on_system_one(&self) -> StubBuilder {
        StubBuilder::new(self.inner.clone(), Endpoint::SystemOne)
    }

    /// Script `GET /v1/models` responses. The stub is registered when the builder is dropped.
    pub fn on_models(&self) -> StubBuilder {
        StubBuilder::new(self.inner.clone(), Endpoint::Models)
    }

    /// Snapshot of received requests, in order.
    #[must_use]
    pub fn journal(&self) -> Vec<RecordedRequest> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .journal
            .clone()
    }

    /// Gaps between consecutive journal timestamps.
    #[must_use]
    pub fn request_gaps(&self) -> Vec<Duration> {
        let journal = self.journal();
        journal
            .windows(2)
            .map(|w| w[1].received_at.saturating_duration_since(w[0].received_at))
            .collect()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

/// Fluent stub registration. Mounted on drop.
pub struct StubBuilder {
    inner: Arc<Mutex<Inner>>,
    stub: Stub,
    mounted: bool,
}

impl StubBuilder {
    fn new(inner: Arc<Mutex<Inner>>, endpoint: Endpoint) -> Self {
        let body = match endpoint {
            Endpoint::SystemOne => json!({
                "model": "jev-latest",
                "answers": {},
                "usage": { "input_tokens": 0, "output_tokens": 0 }
            }),
            Endpoint::Models => json!({ "models": [] }),
        };
        Self {
            inner,
            stub: Stub {
                endpoint,
                matcher: Matcher::Any,
                status: 200,
                headers: Vec::new(),
                body,
                remaining: None,
                delay: None,
            },
            mounted: false,
        }
    }

    /// Only match systemone requests whose `questions` map contains `key`.
    pub fn with_question_key(mut self, key: impl Into<String>) -> Self {
        self.stub.matcher = Matcher::QuestionKey(key.into());
        self
    }

    /// JSON body to return.
    ///
    /// If `body` looks like an answers map (no top-level `answers` / `error` /
    /// `models` keys), it is wrapped as a System One response. Use
    /// [`Self::respond_raw`] to send an exact body.
    pub fn respond(mut self, body: Value) -> Self {
        self.stub.body = wrap_systemone_body(self.stub.endpoint, body);
        self
    }

    /// JSON body to return, without wrapping.
    pub fn respond_raw(mut self, body: Value) -> Self {
        self.stub.body = body;
        self
    }

    /// Set the HTTP status (and a small JSON error body when not 2xx).
    pub fn respond_status(mut self, status: u16) -> Self {
        self.stub.status = status;
        if !(200..300).contains(&status) {
            self.stub.body = json!({ "error": format!("status {status}") });
        }
        self
    }

    /// Add a response header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.stub.headers.push((name.into(), value.into()));
        self
    }

    /// How many times this stub may match. Omit for unlimited.
    pub fn times(mut self, n: u32) -> Self {
        self.stub.remaining = Some(n);
        self
    }

    /// Sleep before responding (latency injection).
    pub fn delay(mut self, delay: Duration) -> Self {
        self.stub.delay = Some(delay);
        self
    }

    fn mount(&mut self) {
        if self.mounted {
            return;
        }
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .stubs
            .push(self.stub.clone());
        self.mounted = true;
    }
}

impl Drop for StubBuilder {
    fn drop(&mut self) {
        self.mount();
    }
}

fn wrap_systemone_body(endpoint: Endpoint, body: Value) -> Value {
    if endpoint != Endpoint::SystemOne {
        return body;
    }
    if body.get("answers").is_some() || body.get("error").is_some() || body.get("models").is_some()
    {
        return body;
    }
    json!({
        "model": "jev-latest",
        "answers": body,
        "usage": { "input_tokens": 0, "output_tokens": 0 }
    })
}

/// Build a Noul answer object.
#[must_use]
pub fn noul(value: f64) -> Value {
    json!({ "type": "noul", "noul": value })
}

/// Build a Choice answer object.
#[must_use]
pub fn choice(label: &str, confidence: f64) -> Value {
    json!({
        "type": "choice",
        "choice": label,
        "probabilities": { label: 1.0 },
        "confidence": confidence
    })
}

/// Build a Score answer object.
#[must_use]
pub fn score(value: f64, confidence: f64) -> Value {
    json!({
        "type": "score",
        "score": value,
        "legend": { "0": "low", "1": "high" },
        "probabilities": { "0": 1.0 - value.min(1.0), "1": value.min(1.0) },
        "confidence": confidence
    })
}

async fn system_one(State(state): State<Arc<Mutex<Inner>>>, req: Request<Body>) -> Response {
    handle(state, Endpoint::SystemOne, req).await
}

async fn models(State(state): State<Arc<Mutex<Inner>>>, req: Request<Body>) -> Response {
    handle(state, Endpoint::Models, req).await
}

async fn fallback() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        axum::Json(json!({ "error": "not found" })),
    )
}

async fn handle(state: Arc<Mutex<Inner>>, endpoint: Endpoint, req: Request<Body>) -> Response {
    let method = req.method().as_str().to_owned();
    let path = req.uri().path().to_owned();
    let header_map = req.headers().clone();
    let (_parts, body) = req.into_parts();
    let bytes = to_bytes(body, 2 * 1024 * 1024).await.unwrap_or_default();
    let json_body = serde_json::from_slice::<Value>(&bytes).ok();

    let mut recorded_headers = HashMap::new();
    for (name, value) in &header_map {
        if let Ok(v) = value.to_str() {
            recorded_headers.insert(name.as_str().to_ascii_lowercase(), v.to_owned());
        }
    }

    let recorded = RecordedRequest {
        method,
        path,
        headers: recorded_headers,
        body: json_body.clone(),
        received_at: Instant::now(),
    };

    let chosen = {
        let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
        guard.journal.push(recorded);
        guard.request_counter += 1;
        let request_id = format!("mock-{}", guard.request_counter);
        let idx = guard.stubs.iter().position(|stub| {
            if stub.endpoint != endpoint {
                return false;
            }
            if stub.remaining == Some(0) {
                return false;
            }
            match &stub.matcher {
                Matcher::Any => true,
                Matcher::QuestionKey(key) => json_body
                    .as_ref()
                    .and_then(|b| b.get("questions"))
                    .and_then(Value::as_object)
                    .is_some_and(|q| q.contains_key(key)),
            }
        });
        idx.map(|i| {
            if let Some(left) = &mut guard.stubs[i].remaining {
                *left = left.saturating_sub(1);
            }
            let mut stub = guard.stubs[i].clone();
            if !stub
                .headers
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case("x-typesafe-request-id"))
            {
                stub.headers
                    .push(("x-typesafe-request-id".to_owned(), request_id));
            }
            stub
        })
    };

    let Some(stub) = chosen else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(json!({ "error": "no stub matched" })),
        )
            .into_response();
    };

    if let Some(delay) = stub.delay {
        tokio::time::sleep(delay).await;
    }

    respond(stub)
}

fn respond(stub: Stub) -> Response {
    let status = StatusCode::from_u16(stub.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = stub.body.to_string();
    let mut headers = AxumHeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    for (name, value) in stub.headers {
        if let (Ok(n), Ok(v)) = (
            axum::http::HeaderName::try_from(name),
            HeaderValue::from_str(&value),
        ) {
            headers.insert(n, v);
        }
    }
    (status, headers, body).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn binds_localhost() {
        let mock = MockServer::start().await;
        assert_eq!(mock.url().scheme(), "http");
        assert_eq!(mock.url().host_str(), Some("127.0.0.1"));
        assert!(mock.url().port().is_some());
    }
}
