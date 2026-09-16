# typesafe-rs: PRD

**A latency-first, production-grade Rust SDK for TypeSafe's System One API, with exact behavioural parity with the official Python and TypeScript SDKs.**

| Field | Value |
|---|---|
| Owner | Abdel Bakhta (@AbdelStark) |
| Status | Draft v1.0, 16 Sep 2026 |
| Repo | `github.com/AbdelStark/typesafe-rs` |
| Crates | `typesafe-rs` (SDK), `typesafe-rs-mock` (test server); both names free on crates.io as of 16 Sep 2026 |
| License | MIT OR Apache-2.0 |
| Target | v0.1 in 2 weeks, v0.2 in 4 weeks, v0.3 in 7 weeks |
| Companion | [`SPEC.md`](./SPEC.md) |
| Downstream | [`s1-rs`](../s1-rs/PRD.md), [`reflex`](../reflex/PRD.md), [`sieve`](../sieve/PRD.md) |

---

## 1. Context

TypeSafe ships official SDKs for Python (`typesafe-sdk`) and TypeScript (`@typesafe-ai/sdk`, v0.6.0, author `evinism`). There is no official Rust SDK.

A community crate, `typesafe-ai` 0.1.0 by `Twister915` (Joey), was published on 16 Sep 2026. It is carefully built with a deliberately minimal scope. Its behaviour also diverges from the official SDKs in ways that matter in production:

| Behaviour | Official Python/TS SDKs | `typesafe-ai` 0.1.0 |
|---|---|---|
| Retryable statuses | 408, 429, all 5xx | 429, 529 |
| Connection errors and timeouts retried | Yes | No |
| Default timeout | 10 s | 60 s per attempt |
| Backoff | 500 ms initial, 5 s cap, 25% jitter; honors `retry-after-ms` and `Retry-After` up to 60 s | 250 ms initial, 8 s cap; honors server delay up to 60 s |
| `GET /v1/models` | Yes | Not covered |
| Env vars `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL`, `TYPESAFE_DEFAULT_MODEL` | Yes | Not covered |
| SDK identification headers (`X-TypeSafe-SDK`, `X-TypeSafe-Runtime`, `X-TypeSafe-Retry-Count`) | Yes | Not covered |

(Official behaviour read from the published `@typesafe-ai/sdk` 0.6.0 package and the Python SDK docs on 16 Sep 2026. Re-verify before implementation.)

**typesafe-rs exists to be the Rust SDK TypeSafe could adopt as official:** identical semantics to its siblings, plus what Rust uniquely enables for System One workloads, namely hot-path latency, high-throughput pipelines, and composable middleware.

## 2. Coexisting with `typesafe-ai` (how to ship this gracefully)

Shipping a second client in launch week is a legitimate choice if it is done openly:

1. **Tell Joey before publishing.** Share the scope and the parity rationale, and credit his crate in the README.
2. **Differentiate on axes, not adjectives.** The comparison table in the README is factual, dated, and links to his crate.
3. **Support both from s1-rs.** `s1-rs` works over typesafe-rs by default and over `typesafe-ai` behind a feature flag, so the typed layer never forces a choice.
4. **Talk to TypeSafe early.** Contact `evinism` (official TS SDK author) and Erik about a shared conformance fixture set. If TypeSafe wants one official Rust SDK, offer to merge efforts or transfer the crate.

## 3. Problem

Rust users of System One today have to choose between writing raw HTTP and adopting a client whose retry, timeout, and configuration semantics differ from TypeSafe's documented SDK behaviour. Separately, the workloads where Rust matters most are underserved by every existing SDK:

- **Hot paths:** 70 ms decisions are wasted if the client adds TLS handshakes, allocations, or head-of-line blocking.
- **Firehose pipelines:** tens to hundreds of requests per second need bounded concurrency, adaptive rate limiting driven by `429` and `Retry-After`, and spend caps.
- **Service stacks:** axum and tonic services compose behaviour through `tower` layers; SDKs that ignore `tower` force duplicate plumbing.
- **Escalation:** the natural architecture is System One first, then a reasoning LLM when confidence is low. No SDK provides that as a primitive.

## 4. Goals

| # | Goal | Milestone |
|---|---|---|
| G1 | Full API coverage: `POST /v1/systemone`, `GET /v1/models` | v0.1 |
| G2 | Behavioural parity with official SDKs (config, env vars, defaults, retries, headers, error taxonomy), proven by a conformance test suite | v0.1 |
| G3 | Async client (tokio) and blocking client | v0.1 |
| G4 | `typesafe-rs-mock`: in-process mock server with scripted answers, fault injection, record and replay | v0.1 |
| G5 | Latency-first transport: persistent HTTP/2, pre-warming, keepalive, per-request timing breakdown | v0.2 |
| G6 | Prepared requests: questions serialized once, state spliced per call | v0.2 |
| G7 | `tower::Service` implementation and layers: concurrency, adaptive rate limit, spend budget | v0.2 |
| G8 | Streaming evaluation over many states with backpressure | v0.2 |
| G9 | LLM backends implementing the same interface (port of `system-one-adapter` semantics) and a `Cascade` backend for confidence-based escalation | v0.3 |
| G10 | WASM target (Cloudflare Workers, browsers) | v0.4 stretch |

## 5. Non-goals

- Typed derive layer (that is `s1-rs`).
- Hidden hedged or duplicate requests by default (opt-in only; they cost money).
- Supporting endpoints not documented by TypeSafe.

## 6. Users

| User | Needs |
|---|---|
| Rust service engineers | Drop-in client that behaves like the docs say |
| Real-time systems (games, trading, edge, networking) | Predictable low overhead, warm connections, timing visibility |
| Data and stream processing | Throughput with safety rails on rate and spend |
| Agent infra builders | Middleware composition, escalation to LLMs |
| TypeSafe | A Rust SDK aligned with its other SDKs, adoptable as official |

## 7. Success criteria

**Correctness:** 100% of conformance fixtures pass; zero divergence from official retry semantics in fault-injection tests.
**Performance:** client-side overhead under 100 µs p50 per warm request excluding network and JSON of the state; zero TLS handshakes per request after warm-up; published benchmarks.
**Adoption:** used in production by s1-rs, Reflex, and Sieve; 5 external dependents within 8 weeks.
**Signal:** TypeSafe links it from docs, contributes fixtures, or discusses adoption.

## 8. Milestones

| Week | Deliverable |
|---|---|
| 0 | Outreach to Joey, evinism, Erik; conformance fixture proposal as a public issue |
| 1 | Types, errors, config and env, retry engine, async client, models endpoint |
| 2 | Blocking client, mock server, conformance suite, docs; **v0.1** |
| 3 | HTTP/2 transport tuning, pre-warm, timing breakdown, prepared requests, benches |
| 4 | tower layers (adaptive rate limit, budget), streaming; **v0.2** |
| 5 to 7 | OpenAI-compatible and Anthropic backends, Cascade; **v0.3** |
| 8+ | WASM exploration |

## 9. Risks

| Risk | Mitigation |
|---|---|
| Perceived as undermining the first community crate | Private note before release, credit in README, s1-rs supports both, factual comparison only |
| Official SDK semantics change | Conformance fixtures versioned; weekly CI job diffs behaviour against the latest official TS package |
| Retrying 5xx on POST duplicates billable work | Match official defaults but document it prominently; `RetryPolicy::conservative()` preset retries only 408/429 and connection errors before bytes are sent |
| LLM backend scope creep | Ship after v0.2, behind features, with a strict parity target (same question and answer shapes as the Python adapter) |
| Early-access key needed for live tests | Public CI uses mock and cassettes; live tests on a private runner |
| Name confusion | Crate is `typesafe-rs`, clearly community until TypeSafe says otherwise |

## 10. Open questions

- Official retry semantics for mid-body failures and ambiguous timeouts (duplicate risk).
- Maximum questions per request and payload size limits.
- Whether TypeSafe publishes an OpenAPI document to generate fixtures from.
- Rate-limit headers beyond `Retry-After` (for example remaining quota), if any.
