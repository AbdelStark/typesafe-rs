# typesafe-rs

Rust SDK for TypeSafe's System One API. Read `SPEC.md` before changing behaviour marked **[parity]**.

## Build and test

```bash
cargo test --workspace
cargo test --workspace --all-features
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo run -p typesafe-rs --example quickstart
```

MSRV is **1.85** (edition 2024). Use the stable toolchain only.

Do not add CI that calls `https://api.typesafe.ai`.

## Crate map

| Crate | Path | Role |
|---|---|---|
| `typesafe-rs` | `crates/typesafe-rs` | SDK (`Client`, `BlockingClient`, wire types, retry, errors). `#![forbid(unsafe_code)]` |
| `typesafe-rs-mock` | `crates/typesafe-rs-mock` | In-process HTTP mock (`MockServer`) |

Typed derive over enums is out of scope.

## Parity

**[parity]** in `SPEC.md` must match the official Python SDK and `@typesafe-ai/sdk` 0.6.0:

- Env: `TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL`, `TYPESAFE_DEFAULT_MODEL`
- Defaults: timeout 10s, model `jev-latest`, base `https://api.typesafe.ai`
- Retry: max 2, 500ms initial, 5s cap, 25% jitter; 408/429/5xx; connection errors and timeouts; `retry-after-ms` then `Retry-After` up to 60s
- Headers: `Authorization`, `Accept`, `User-Agent: typesafe-rs/<version>`, `X-TypeSafe-SDK`, `X-TypeSafe-Runtime: rust/<rustc>; <os>-<arch>`, `X-TypeSafe-Retry-Count` on retries only (n starting at 1)

## Fixtures

`conformance/fixtures/*.json` are executed by `crates/typesafe-rs/tests/conformance.rs` against the real mock server. See `conformance/README.md`.

HTTP retry tests must use `typesafe-rs-mock`, not an in-memory stub of `Client`. Delay assertions go through `RetryPolicy::delay_after_failure` / `delay_after_failure_with`, not a reimplemented formula.

## v0.1 vs later

**Shipped in v0.1:** async + blocking clients, `Backend` trait, mock server, conformance fixtures, `GET /v1/models`, `POST /v1/systemone`.

**Not in v0.1:** tower layers, `evaluate_stream`, prepared requests, LLM backends, Cascade, WASM, cargo-deny CI, TypeScript parity script, live API tests.

CI (fmt, clippy, tests, rustdoc, MSRV) lives in `.github/workflows/ci.yml`.

Transport is **reqwest 0.13 + rustls** (see `ROADMAP.md`). Hyper 1.x remains a v0.2 option.
