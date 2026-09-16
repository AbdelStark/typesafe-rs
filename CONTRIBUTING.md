# Contributing

## Setup

Stable Rust **1.85+** (edition 2024). Do not use nightly.

```bash
git clone https://github.com/AbdelStark/typesafe-rs
cd typesafe-rs
cargo test --workspace --all-features
```

## Checks

Run these before opening a PR:

```bash
cargo test --workspace
cargo test --workspace --all-features
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo run -p typesafe-rs --example quickstart
```

Do not add CI that calls `https://api.typesafe.ai`.

## Layout

| Path | Role |
|---|---|
| `crates/typesafe-rs` | SDK |
| `crates/typesafe-rs-mock` | In-process HTTP mock |
| `conformance/fixtures` | JSON scripts run by `crates/typesafe-rs/tests/conformance.rs` |
| `SPEC.md` | Behaviour, including **[parity]** with the official Python and TypeScript SDKs |
| `ROADMAP.md` | What shipped in v0.1 and what is later |

## Parity

Behaviour marked **[parity]** in `SPEC.md` must match the official Python SDK and `@typesafe-ai/sdk` (currently 0.6.0): env vars, defaults, retries, headers, and error kinds.

HTTP retry tests must use `typesafe-rs-mock`, not an in-memory stub of `Client`. Delay assertions go through `RetryPolicy::delay_after_failure` / `delay_after_failure_with`.

## Scope

This crate is the HTTP client and mock. Out of scope: typed derive over enums, undocumented endpoints, hidden hedged or duplicate requests by default.

See `ROADMAP.md` for work that is explicitly later than v0.1 (tower layers, streaming, prepared requests, LLM backends, WASM).

## License

Contributions are dual-licensed MIT OR Apache-2.0, the same as the rest of the repo.
