# typesafe-rs: PRD

Rust SDK for TypeSafe's System One API: `POST /v1/systemone` and `GET /v1/models`, with the same config, retry, header, and error behaviour as the official Python and TypeScript SDKs.

| Field | Value |
|---|---|
| Owner | Abdel Bakhta (@AbdelStark) |
| Status | v0.1 shipped, 16 Sep 2026 |
| Repo | `github.com/AbdelStark/typesafe-rs` |
| Crates | `typesafe-rs` (SDK), `typesafe-rs-mock` (test server) |
| License | MIT OR Apache-2.0 |
| Companion | [`SPEC.md`](./SPEC.md) |

---

## 1. Problem

TypeSafe documents System One as: evaluate a `state` against a map of typed questions, get one answer per question. Official clients exist for Python (`typesafe-sdk`) and TypeScript (`@typesafe-ai/sdk`).

Rust callers need a client that:

- Matches those SDKs on env vars, defaults, retries, identification headers, and error kinds.
- Provides async and blocking APIs.
- Can be tested without hitting `https://api.typesafe.ai`.

Later work (see `ROADMAP.md`) covers hot-path transport tuning, prepared requests, tower layers, streaming, and pluggable backends.

## 2. Goals

| # | Goal | Milestone |
|---|---|---|
| G1 | API coverage: `POST /v1/systemone`, `GET /v1/models` | v0.1 |
| G2 | Behavioural parity with official SDKs (config, env vars, defaults, retries, headers, error taxonomy), proven by a conformance test suite | v0.1 |
| G3 | Async client (tokio) and blocking client | v0.1 |
| G4 | `typesafe-rs-mock`: in-process mock server with scripted answers, fault injection, and a request journal | v0.1 |
| G5 | Latency-first transport: persistent HTTP/2, pre-warming, keepalive, per-request timing breakdown | v0.2 |
| G6 | Prepared requests: questions serialized once, state spliced per call | v0.2 |
| G7 | `tower::Service` implementation and layers: concurrency, adaptive rate limit, spend budget | v0.2 |
| G8 | Streaming evaluation over many states with backpressure | v0.2 |
| G9 | LLM backends implementing the same interface and a `Cascade` backend for confidence-based escalation | v0.3 |
| G10 | WASM target (Cloudflare Workers, browsers) | v0.4 stretch |

## 3. Non-goals

- Typed derive layer (enums → questions).
- Hidden hedged or duplicate requests by default (opt-in only; they cost money).
- Supporting endpoints TypeSafe does not document.

## 4. Users

| User | Needs |
|---|---|
| Rust service engineers | Client that matches the documented SDK behaviour |
| Real-time systems | Predictable overhead, warm connections, timing visibility |
| Data and stream processing | Throughput with rate and spend limits |
| Agent infra | Middleware composition, escalation to LLMs |

## 5. Success criteria

**Correctness:** conformance fixtures pass; retry behaviour matches the official SDKs in fault-injection tests.

**Performance (v0.2):** client-side overhead under 100 µs p50 per warm request excluding network and JSON of the state; zero TLS handshakes per request after warm-up; published benchmarks.

## 6. Milestones

| Week | Deliverable |
|---|---|
| 1–2 | Types, errors, config and env, retry engine, async + blocking clients, mock, conformance; **v0.1** |
| 3–4 | HTTP/2 transport tuning, pre-warm, timing breakdown, prepared requests, tower layers, streaming; **v0.2** |
| 5–7 | OpenAI-compatible and Anthropic backends, Cascade; **v0.3** |
| 8+ | WASM exploration |

## 7. Risks

| Risk | Mitigation |
|---|---|
| Official SDK semantics change | Versioned conformance fixtures; later, a runner against `@typesafe-ai/sdk` |
| Retrying 5xx on POST duplicates billable work | Match official defaults; document it; `RetryPolicy::conservative()` retries only 408/429 and connection errors before bytes are sent |
| Early-access key needed for live tests | Public CI uses the mock; live tests stay off CI |

## 8. Open questions

- Official retry semantics for mid-body failures and ambiguous timeouts (duplicate risk).
- Maximum questions per request and payload size limits.
- Whether TypeSafe publishes an OpenAPI document to generate fixtures from.
- Rate-limit headers beyond `Retry-After` (for example remaining quota), if any.
