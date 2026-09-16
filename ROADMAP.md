# Roadmap

Current status: **v0.1 shipped (MVP)** — 16 Sep 2026.

## v0.1 MVP — shipped

- [x] Cargo workspace, edition 2024, MSRV 1.85
- [x] `typesafe-rs` async `Client` (`POST /v1/systemone`, `GET /v1/models`, `warm_up`)
- [x] Feature `blocking`: `BlockingClient`
- [x] Wire types: `Entry`, `Question` (Noul/Choice/Score), `SystemOneRequest`/`Response`, `ModelCard`
- [x] Unknown answer types do not fail deserialize
- [x] Client-side validation (Choice 2..=255, Score >= 2, non-empty question map/keys)
- [x] Config/env parity: `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL`, `TYPESAFE_DEFAULT_MODEL`
- [x] Retry parity (408/429/5xx, connection, timeout, `retry-after-ms` / `Retry-After`)
- [x] `RetryPolicy::none()` and `RetryPolicy::conservative()`
- [x] SDK identification headers + `X-TypeSafe-Retry-Count` on retries
- [x] Error taxonomy; Display never includes API keys or request bodies
- [x] Thin `Backend` trait (`impl Backend for Client`)
- [x] `typesafe-rs-mock` in-process server (scripted answers, 429 sequences, journal)
- [x] Conformance fixtures + runner against the real client and mock
- [x] `examples/quickstart.rs` against the mock
- [x] Dual license MIT OR Apache-2.0

### Transport choice (v0.1)

SPEC prefers hyper 1.x + hyper-util. v0.1 ships **reqwest 0.13** with the `rustls` feature (HTTP/2, `TCP_NODELAY`, 90s pool idle, HTTP/2 keepalive 30s/10s, adaptive window). Reqwest still hits a real HTTP stack; retry, headers, and TLS are not stubbed.

Reqwest 0.13's `rustls` feature uses rustls with the **platform verifier** rather than webpki-only roots. Revisit webpki-only roots if TypeSafe requires a fixed root store.

## v0.2 — HTTP/2 tuning, prepared, tower, stream

- [ ] Replace or wrap reqwest with hyper 1.x if pooling/H2 tuning needs it
- [ ] Per-request timing breakdown (feature `timing`)
- [ ] Prepared requests (serialize questions once, splice `state`)
- [ ] `tower::Service` + `AdaptiveRateLimitLayer` + `BudgetLayer`
- [ ] `evaluate_stream` with bounded concurrency (feature `stream`)
- [ ] Published benches vs `typesafe-ai` 0.1.0 on the same mock
- [ ] `cargo deny` / MSRV CI (no live API)

## v0.3 — LLM backends and Cascade

- [ ] `Backend` implementations: OpenAI-compatible, Anthropic
- [ ] `Cascade` (System One, then LLM on low confidence)
- [ ] Port of `system-one-adapter` semantics

## Later / out of scope for now

- [ ] WASM target (v0.4 stretch)
- [ ] TypeScript fixture runner (`conformance/ts/run.mjs`) against `@typesafe-ai/sdk`
- [ ] Live API smoke tests on a private runner
- [ ] Outreach to Joey / evinism / Erik (deferred while the repo is private)

## Explicit non-goals

- Typed derive layer (that is `s1-rs`)
- Hidden hedged or duplicate requests by default
- Endpoints TypeSafe does not document
