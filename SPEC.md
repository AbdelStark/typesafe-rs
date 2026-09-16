# typesafe-rs: Technical Specification

Companion to [`PRD.md`](./PRD.md). Before implementation, re-read `https://docs.typesafe.ai/api`, `https://docs.typesafe.ai/primitives/advanced`, the Python SDK reference, and the source of the latest `@typesafe-ai/sdk` npm package. Behaviour marked **[parity]** must match the official SDKs exactly.

---

## 1. Workspace

v0.1 ships the files under `crates/`. Modules marked v0.2 / v0.3 are the target layout, not files on disk yet.

```
typesafe-rs/
  Cargo.toml                    # workspace, edition 2024, resolver 3
  crates/
    typesafe-rs/                # the SDK
      src/
        lib.rs
        config.rs               # ClientConfig, env loading
        types/
          request.rs            # Request, Question (Noul/Choice/Score), Entry
          response.rs           # SystemOneResponse, Answer, Usage, typed views
          models.rs             # ModelCard, ListModelsResponse
        error.rs                # Error taxonomy
        retry.rs                # RetryPolicy, delay calculation, Retry-After parsing
        transport.rs            # v0.1: reqwest 0.13 + rustls
        client.rs               # Client (async)
        blocking.rs             # BlockingClient (feature "blocking")
        backend.rs              # Backend trait (`impl Backend for Client`)
        prepared.rs             # PreparedQuestions (v0.2)
        stream.rs               # evaluate_stream (v0.2, feature "stream")
        tower/                  # Service impl + layers (v0.2, feature "tower")
          service.rs
          rate.rs               # AdaptiveRateLimitLayer
          budget.rs             # BudgetLayer
        backends/               # v0.3
          mod.rs
          openai_compat.rs      # feature "llm-openai"
          anthropic.rs          # feature "llm-anthropic"
          cascade.rs
    typesafe-rs-mock/           # axum mock server
  conformance/
    fixtures/*.json             # request/response/behaviour fixtures
    runner/                     # Rust test harness; TS parity script in /conformance/ts
  benches/
  examples/
```

MSRV 1.85 (edition 2024). Stable toolchain only.

### Features

| Feature | Default | Enables |
|---|---|---|
| `rustls` | yes | TLS via rustls with webpki roots |
| `native-tls` | no | Platform TLS instead |
| `blocking` | no | `BlockingClient` |
| `stream` | no | `evaluate_stream` |
| `tower` | no | `tower::Service` + layers |
| `timing` | no | Per-request timing breakdown |
| `llm-openai`, `llm-anthropic` | no | LLM backends |
| `tracing` | yes | spans and events |

## 2. Wire types

### 2.1 Entry

Every `instructions` value and criteria description accepts `string | object | array | null`.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Entry { Text(Cow<'static, str>), Json(serde_json::Value), Null }
impl From<&'static str> for Entry { ... }
impl From<String> for Entry { ... }
impl From<serde_json::Value> for Entry { ... }
```

### 2.2 Questions

```rust
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    Noul   { instructions: Entry, #[serde(skip_serializing_if = "Option::is_none")] criteria: Option<NoulCriteria> },
    Choice { instructions: Entry, criteria: IndexMap<String, Entry> },   // insertion order preserved
    Score  { instructions: Entry, criteria: Vec<Entry> },                 // index = level
}

#[derive(Clone, Debug, Serialize)]
pub struct NoulCriteria {
    #[serde(rename = "true",  skip_serializing_if = "Option::is_none")] pub when_true: Option<Entry>,
    #[serde(rename = "false", skip_serializing_if = "Option::is_none")] pub when_false: Option<Entry>,
}

// Builders
Question::noul("Does this convey urgency?").when_true("Explicitly time-sensitive").when_false("No time pressure");
Question::choice("Which team?").option("billing", "Payment issues").option("technical", "Bugs");
Question::score("How frustrated?").level("Calm").level("Frustrated").level("Very angry");
```

Client-side validation (returns `Error::InvalidRequest` before any network call): Choice has at least 2 options and at most 255 (constant `MAX_CHOICE_OPTIONS`, verify); Score has at least 2 levels; question map non-empty; keys non-empty.

### 2.3 Request

```rust
#[derive(Clone, Debug, Serialize)]
pub struct SystemOneRequest {
    pub state: serde_json::Value,                 // string, object, or array
    pub model: String,                            // default from config
    pub questions: IndexMap<String, Question>,
}
```

### 2.4 Response

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct SystemOneResponse {
    pub model: String,
    pub answers: IndexMap<String, Answer>,
    #[serde(default)] pub usage: Option<Usage>,
    #[serde(skip)] pub meta: ResponseMeta,        // request_id, status, headers, attempts, timing, raw bytes (opt-in)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Answer {
    Noul   { noul: f64 },
    Choice { choice: String, probabilities: IndexMap<String, f64>, confidence: f64 },
    Score  { score: f64, legend: IndexMap<String, serde_json::Value>,
             #[serde(default)] probabilities: Option<IndexMap<String, f64>>, confidence: f64 },
    #[serde(other)] Unknown,                      // forward compatibility
}

impl SystemOneResponse {
    pub fn nouls(&self)   -> impl Iterator<Item = (&str, NoulView<'_>)>;   // parity with Python `.nouls`
    pub fn choices(&self) -> impl Iterator<Item = (&str, ChoiceView<'_>)>;
    pub fn scores(&self)  -> impl Iterator<Item = (&str, ScoreView<'_>)>;
    pub fn noul(&self, key: &str)   -> Option<f64>;
    pub fn choice(&self, key: &str) -> Option<ChoiceView<'_>>;
    pub fn score(&self, key: &str)  -> Option<ScoreView<'_>>;
}
```

Unknown answer types or extra fields never fail deserialization.

### 2.5 Models

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct ModelCard { pub name: String, pub description: String, pub release_date: String }
#[derive(Clone, Debug, Deserialize)]
pub struct ListModelsResponse { pub models: Vec<ModelCard> }
```

A body without `models` array returns `Error::UnexpectedShape` **[parity]**.

## 3. Configuration **[parity]**

```rust
pub struct ClientConfig {
    pub api_key: Option<SecretString>,     // falls back to TYPESAFE_API_KEY
    pub base_url: Option<Url>,             // falls back to TYPESAFE_BASE_URL, then https://api.typesafe.ai
    pub default_model: Option<String>,     // falls back to TYPESAFE_DEFAULT_MODEL, then "jev-latest"
    pub timeout: Duration,                 // default 10 s, per attempt including body
    pub retry: RetryPolicy,
    pub default_headers: HeaderMap,        // cannot override Authorization, SDK identification, Accept
    pub transport: TransportConfig,
}
```

Precedence: explicit code value, then environment, then default. Missing API key at construction returns `Error::MissingApiKey` with the env var name in the message.

Per-call overrides via `CallOptions { timeout, retry, headers, model }`.

## 4. HTTP contract **[parity]**

- `POST {base}/v1/systemone`, body JSON, `Content-Type: application/json`.
- `GET {base}/v1/models`.
- Headers on every attempt: `Authorization: Bearer <key>`, `Accept: application/json`, `User-Agent: typesafe-rs/<version>`, `X-TypeSafe-SDK: typesafe-rs/<version>`, `X-TypeSafe-Runtime: rust/<rustc version>; <os>-<arch>`.
- `X-TypeSafe-Retry-Count: <n>` on retry attempts only (n starting at 1).
- Base URL path prefixes preserved.
- `x-typesafe-request-id` captured into `meta.request_id` on success and error.

The SDK identifier is `typesafe-rs/<version>` (not `typesafe-sdk`) so traffic is attributable to this crate.

## 5. Retry engine **[parity]**

```rust
pub struct RetryPolicy {
    pub max_retries: u32,                  // default 2
    pub backoff_initial: Duration,         // 500 ms
    pub backoff_max: Duration,             // 5 s
    pub backoff_jitter: f64,               // 0.25 (fraction subtracted)
    pub http_statuses: StatusSet,          // 408, 429, 500..=599
    pub respect_retry_after: bool,         // true
    pub max_retry_after: Duration,         // 60 s
    pub retry_connection_errors: bool,     // true
    pub retry_timeouts: bool,              // true
}
impl RetryPolicy {
    pub fn none() -> Self;
    pub fn conservative() -> Self;         // 408, 429, connection errors before request bytes sent; no 5xx, no timeouts
}
```

Delay for zero-based attempt `a`:

```
if respect_retry_after and header delay d exists and d <= max_retry_after: return d
exp = min(backoff_initial * 2^a, backoff_max)
return round(exp * (1 - U[0,1) * backoff_jitter))
```

Header parsing: prefer `retry-after-ms` (non-negative finite number); else `Retry-After` as seconds (non-negative) or HTTP date (clamped at 0). Invalid values ignored.

Caller cancellation (dropping the future) stops further attempts immediately; no background retries.

Every retry emits a `tracing` event and increments `meta.attempts`.

## 6. Error taxonomy **[parity with TS classes]**

```rust
#[non_exhaustive]
pub enum Error {
    MissingApiKey,
    InvalidRequest(String),
    Connection(TransportError),                // APIConnectionError
    Timeout { after: Duration },               // APITimeoutError
    Api(ApiError),                             // non-2xx
    Decode { source: serde_json::Error, body: Bytes, meta: ResponseMeta },
    UnexpectedShape { endpoint: &'static str, meta: ResponseMeta },
}

pub struct ApiError { pub status: StatusCode, pub kind: ApiErrorKind, pub body: ErrorBody, pub request_id: Option<String>, pub headers: HeaderMap, pub endpoint: String }

pub enum ApiErrorKind { BadRequest /*400*/, Authentication /*401*/, PermissionDenied /*403*/, NotFound /*404*/, Conflict /*409*/, UnprocessableEntity /*422*/, RateLimit /*429*/, InternalServer /*5xx*/, Other }
pub enum ErrorBody { Json(serde_json::Value), Text(String), Empty }
```

Display implementations never include API keys or request bodies.

## 7. Clients

```rust
pub struct Client { inner: Arc<Inner> }             // cheap to clone, Send + Sync

impl Client {
    pub fn new(config: ClientConfig) -> Result<Self, Error>;
    pub fn from_env() -> Result<Self, Error>;
    pub async fn system_one(&self, state: impl Serialize, questions: Questions) -> Result<SystemOneResponse, Error>;
    pub async fn system_one_with(&self, req: &SystemOneRequest, opts: CallOptions) -> Result<SystemOneResponse, Error>;
    pub fn models(&self) -> Models<'_>;             // .list().await
    pub async fn warm_up(&self) -> Result<(), Error>; // establishes pooled HTTP/2 connection (GET /v1/models)
}

#[cfg(feature = "blocking")]
pub struct BlockingClient { client: Client, rt: tokio::runtime::Runtime /* current_thread */ }
```

`Questions` is `IndexMap<String, Question>` with a `questions! { "urgent" => Question::noul("..."), ... }` macro.

## 8. Transport (v0.1 baseline, tuned in v0.2)

v0.1 default: `reqwest` 0.13 with the `rustls` feature (HTTP/2). Reqwest's `rustls` feature uses rustls with the **platform verifier**. Hyper 1.x remains a v0.2 option if pooling or H2 tuning needs it.

| Setting | Value | Why |
|---|---|---|
| ALPN | h2, http/1.1 | multiplex many concurrent decisions on one connection |
| `TCP_NODELAY` | on | small request bodies, avoid Nagle delay |
| Pool idle timeout | 90 s | keep connections warm between bursts |
| HTTP/2 keepalive ping | every 30 s while idle, 10 s timeout | detect dead connections before a hot-path call does |
| HTTP/2 adaptive window | on | large states |
| Happy Eyeballs | on | IPv4/IPv6 |
| Request compression | off by default; `gzip` opt-in if TypeSafe accepts `Content-Encoding` (verify) | large states |

`Transport` trait allows custom HTTP implementations and test doubles.

### 8.1 Timing (feature `timing`)

Instrumented connector records per attempt: `dns`, `tcp_connect`, `tls_handshake` (zero when pooled), `request_write`, `ttfb`, `body_read`, `total`, plus `x-envoy-upstream-service-time` if present. Exposed as `meta.timing: Vec<AttemptTiming>`.

## 9. Prepared requests (v0.2)

For pipelines sending the same questions with different states:

```rust
let prepared = client.prepare(questions)?;         // validates + serializes questions once
let resp = prepared.run(&state).await?;
```

Implementation: pre-serialize `,"model":"...","questions":{...}}` into `Bytes`; per call write `{"state":` + serialized state + prefix bytes into a pooled `BytesMut`. Target: one allocation per call for the body.

## 10. Streaming (v0.2, feature `stream`)

```rust
pub fn evaluate_stream<S, I>(&self, prepared: &Prepared, states: I, opts: StreamOpts)
    -> impl Stream<Item = (usize, Result<SystemOneResponse, Error>)>
where S: Serialize, I: Stream<Item = S>;

pub struct StreamOpts { pub concurrency: usize, pub ordered: bool }
```

Backpressure: pulls from input only when an in-flight slot frees.

## 11. tower integration (v0.2, feature `tower`)

```rust
impl tower::Service<SystemOneRequest> for Client { type Response = SystemOneResponse; type Error = Error; ... }
```

### 11.1 `AdaptiveRateLimitLayer`

AIMD on requests per second:
- Start at `initial_rps`; every 10 s without 429, increase by `additive_step` up to `max_rps`.
- On 429: multiply by `decrease_factor` (0.5), respect `Retry-After` as a global pause.
- Token bucket enforces current rate; `poll_ready` returns `Pending` when empty.

### 11.2 `BudgetLayer`

- Tracks `usage.input_tokens` (and output tokens if ever billed) times configured prices.
- Windows: per minute, per day (UTC).
- Modes on exhaustion: `Reject` (returns `Error::BudgetExceeded`), `Wait` (until window resets), `Callback` (user decides, e.g. sampling in Sieve).
- Exposes `BudgetHandle` for dashboards.

### 11.3 Standard tower layers documented

`ConcurrencyLimitLayer`, `LoadShedLayer`, `TimeoutLayer` examples in docs.

## 12. Backends and Cascade (v0.3)

```rust
#[async_trait::async_trait]      // or native async fn in traits (stable since 1.75)
pub trait Backend: Send + Sync {
    async fn system_one(&self, req: &SystemOneRequest, opts: &CallOptions) -> Result<SystemOneResponse, Error>;
    fn name(&self) -> &str;
}
```

### 12.1 LLM backends

Port of `system-one-adapter` semantics (read its source and README before implementation):
- Build a JSON schema from the question map; use provider structured output when available, else prompt for JSON and validate.
- `AnswerMode::Probabilities` (per-option distribution) or `Discrete`.
- `normalize_probabilities` rescales invalid distributions.
- Corrective retries on schema validation failure (`n_retry_malformed_structure`, default 2).
- Response carries `meta.backend = "openai-compat:<model>"`, token totals across retries, malformed retry count.
- `openai_compat` supports any `/v1/chat/completions` or Responses API base URL (Mistral, vLLM, OpenAI).
- `anthropic` uses the Messages API with tool-based structured output.

Goal: identical inputs and output shapes to TypeSafe, so the same request and response types work against either.

### 12.2 Cascade

```rust
let cascade = Cascade::new(typesafe_client)
    .escalate_to(anthropic_backend)
    .when(|resp| resp.min_confidence() < 0.6)          // or per-key predicates
    .escalate_keys_only(true);                           // re-ask only low-confidence questions
```

- Merges answers: escalated keys replaced, others kept; `meta.escalations` lists keys and reasons.
- Metrics: escalation rate, added latency, added cost.

## 13. Mock server (`typesafe-rs-mock`)

```rust
let mock = MockServer::start().await;
mock.on_system_one()
    .with_question_key("urgent")
    .respond(answers! { "urgent" => noul(0.97) })
    .times(1);
mock.on_system_one().respond_status(429).header("retry-after-ms", "120").times(2);
let client = Client::new(ClientConfig { base_url: Some(mock.url()), api_key: Some("test".into()), ..Default::default() })?;
```

Features: request matchers (key presence, state JSON pointer equality), sequential responses, latency injection, connection reset injection, mid-body truncation, request journal assertions, record and replay to JSON cassettes with key redaction.

## 14. Conformance suite

`conformance/fixtures/*.json`, each:

```json
{
  "id": "retry-429-retry-after-ms",
  "config": { "retry": { "max_retries": 2 } },
  "script": [
    { "status": 429, "headers": { "retry-after-ms": "120" } },
    { "status": 200, "body_file": "ok_noul.json" }
  ],
  "expect": {
    "result": "ok",
    "attempts": 2,
    "delays_ms": [120],
    "retry_count_headers": [null, "1"]
  }
}
```

Coverage: all retry statuses, non-retryable statuses per error kind, Retry-After variants (ms, seconds, HTTP date, invalid, above max), timeouts, connection errors, env precedence, header protection, models shape errors, unknown answer types.

`conformance/ts/run.mjs` (later) executes the same fixtures against the official `@typesafe-ai/sdk` using a local HTTP server, so parity claims are tested, not asserted.

## 15. Observability

- Span `typesafe.request` with `http.request.method`, `url.path`, `server.address`, `typesafe.model`, `typesafe.questions.count`, `typesafe.request_id`, `http.response.status_code`, `typesafe.attempts`, `typesafe.usage.input_tokens`.
- Events: `retry_scheduled { attempt, delay_ms, reason }`, `budget_exceeded`, `escalated { keys }`.
- State and question contents never logged unless `ClientConfig::log_payloads(true)`; API key always redacted.

## 16. Benchmarks (published in README)

| Bench | Method |
|---|---|
| Encode + decode overhead (8 questions, 1 KB state) | criterion, in-memory transport |
| Prepared vs unprepared request build | criterion |
| Warm vs cold request latency | mock server with fixed 50 ms latency over loopback TLS |
| Throughput with AIMD against mock 429 behaviour | custom harness, reports converged rps |

## 17. Quality gates

- `cargo fmt`, `clippy -D warnings`, `cargo test --all-features`, `cargo hack --each-feature`, MSRV check, `cargo deny`, `cargo semver-checks` from v0.2.
- `#![forbid(unsafe_code)]` in `typesafe-rs`.
- 100% documented public items (`#![deny(missing_docs)]`).
- Examples: `quickstart.rs`, `triage.rs`, `models.rs`, `pipeline_stream.rs`, `axum_service.rs`, `cascade.rs`.

## 18. v0.1 release checklist

- [x] Conformance suite green in Rust against the mock
- [x] Mock server documented with retry and failure examples
- [ ] TypeScript fixture runner against `@typesafe-ai/sdk` (later)
- [ ] Live smoke test on a private runner for `system_one` and `models.list`
