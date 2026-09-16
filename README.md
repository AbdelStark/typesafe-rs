# typesafe-rs

Rust client for [TypeSafe](https://typesafe.ai) [System One](https://docs.typesafe.ai/api).

[![Crates.io](https://img.shields.io/crates/v/typesafe-rs.svg)](https://crates.io/crates/typesafe-rs)
[![Docs.rs](https://docs.rs/typesafe-rs/badge.svg)](https://docs.rs/typesafe-rs)
[![CI](https://github.com/AbdelStark/typesafe-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/AbdelStark/typesafe-rs/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85+-blue.svg)](https://blog.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

Evaluate a `state` against named questions (`noul`, `choice`, `score`) and get one typed answer per question.

This is a community SDK. It is not an official TypeSafe product. Env vars, defaults, retries, identification headers, and error kinds match the official Python SDK and [`@typesafe-ai/sdk`](https://www.npmjs.com/package/@typesafe-ai/sdk).

## Install

```toml
[dependencies]
typesafe-rs = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```toml
# optional: blocking client
typesafe-rs = { version = "0.1", features = ["blocking"] }
```

```toml
# tests: in-process mock of POST /v1/systemone and GET /v1/models
[dev-dependencies]
typesafe-rs-mock = "0.1"
```

MSRV is **1.85** (edition 2024). TLS is rustls by default (`native-tls` is available).

## Quick start

```bash
export TYPESAFE_API_KEY=...
```

```rust
use typesafe_rs::{questions, Client, Question};

#[tokio::main]
async fn main() -> Result<(), typesafe_rs::Error> {
    let client = Client::from_env()?;

    let response = client
        .system_one(
            "Help! My payouts have been failing for 3 days.",
            questions! {
                "urgent" => Question::noul("Does this convey urgency?")
                    .when_true("Explicitly time-sensitive")
                    .when_false("No time pressure"),
                "team" => Question::choice("Which team should handle this?")
                    .option("billing", "Payments, invoicing, refunds")
                    .option("technical", "Bugs, outages, integrations")
                    .option("sales", "Pricing, upgrades, new accounts"),
                "frustration" => Question::score("How frustrated is the customer?")
                    .level("Calm")
                    .level("Frustrated")
                    .level("Very angry"),
            },
        )
        .await?;

    println!("urgent      = {:?}", response.noul("urgent"));
    println!("team        = {:?}", response.choice("team").map(|c| c.choice));
    println!("frustration = {:?}", response.score("frustration").map(|s| s.score));
    Ok(())
}
```

`state` can be a string, a JSON object, or an array. Keys you pick on the question map come back on `answers`.

List models:

```rust
let models = client.models().list().await?;
```

Call `client.warm_up().await?` once at process start if you want the first real request to reuse a pooled connection.

## Configuration

`Client::from_env()` and `Client::new` resolve fields as **code, then env, then default**.

| Setting | Env | Default |
|---|---|---|
| API key | `TYPESAFE_API_KEY` | required |
| Base URL | `TYPESAFE_BASE_URL` | `https://api.typesafe.ai` |
| Model | `TYPESAFE_DEFAULT_MODEL` | `jev-latest` |
| Timeout | — | 10 s per attempt, including the body |

```rust
use std::time::Duration;
use typesafe_rs::{ClientConfig, RetryPolicy};

let client = ClientConfig::new()
    .api_key("sk-...")
    .default_model("jev-latest")
    .timeout(Duration::from_secs(15))
    .retry(RetryPolicy::conservative())
    .build()?;
```

Per-call overrides: `CallOptions` (`timeout`, `retry`, `headers`, `model`) with `system_one_with`. `Authorization`, `Accept`, `User-Agent`, `X-TypeSafe-SDK`, `X-TypeSafe-Runtime`, and `X-TypeSafe-Retry-Count` cannot be overridden.

Every request sends:

- `Authorization: Bearer <key>`
- `Accept: application/json`
- `User-Agent: typesafe-rs/<version>`
- `X-TypeSafe-SDK: typesafe-rs/<version>`
- `X-TypeSafe-Runtime: rust/<rustc>; <os>-<arch>`
- `X-TypeSafe-Retry-Count: <n>` on retries only (`n` starts at 1)

API keys are redacted in `Debug`. `Error` display never includes the key or the request body.

## Retries

Default policy matches the official SDKs:

| Property | Value |
|---|---|
| Max retries | 2 (3 attempts total) |
| Retry on | HTTP 408, 429, all 5xx; connection errors; timeouts |
| Backoff | 500 ms initial, 5 s cap, 25% jitter |
| Server delay | `retry-after-ms`, then `Retry-After` (seconds or HTTP date), capped at 60 s |

Presets:

- `RetryPolicy::default()`: official behaviour
- `RetryPolicy::none()`: no retries
- `RetryPolicy::conservative()`: 408/429 and pre-send connection errors only (no 5xx, no timeouts)

Dropping the future cancels further attempts. Retrying 5xx on `POST /v1/systemone` can duplicate billable work; use `conservative()` if that matters more than completing the call.

## Errors

```rust
match client.system_one(state, questions).await {
    Ok(response) => { let _ = response.noul("urgent"); }
    Err(err) => {
        eprintln!("{} (request id {:?})", err, err.request_id());
        if let Some(status) = err.status() {
            eprintln!("HTTP {status}");
        }
    }
}
```

| Variant | When |
|---|---|
| `MissingApiKey` | No key in config or `TYPESAFE_API_KEY` |
| `InvalidRequest` | Client-side validation (empty questions, Choice with fewer than 2 options, Score with fewer than 2 levels, …) before any network call |
| `Connection` | DNS, TLS, or connect failure |
| `Timeout` | Attempt exceeded the timeout |
| `Api` | Non-2xx after retries are exhausted (`401` → `Authentication`, `429` → `RateLimit`, `5xx` → `InternalServer`, …) |
| `Decode` | Body is not JSON of the expected type |
| `UnexpectedShape` | JSON parsed but is missing a documented field (for example `GET /v1/models` without `models`) |

Unknown answer `type` values deserialize as `Answer::Unknown` instead of failing.

## Testing

`typesafe-rs-mock` is an in-process HTTP server. Tests talk to a real `Client` over loopback, not a stub of the SDK.

```rust
use typesafe_rs::{questions, ClientConfig, Question};
use typesafe_rs_mock::{noul, MockServer};

let mock = MockServer::start().await;
mock.on_system_one().respond(serde_json::json!({
    "urgent": noul(0.97),
}));

let client = ClientConfig::new()
    .api_key("test")
    .base_url(mock.url())
    .build()?;

let response = client
    .system_one(
        "Help! My payouts have been failing for 3 days.",
        questions! { "urgent" => Question::noul("Does this convey urgency?") },
    )
    .await?;

assert_eq!(response.noul("urgent"), Some(0.97));
```

Script a 429 then a success, including `retry-after-ms`:

```rust
mock.on_system_one()
    .respond_status(429)
    .header("retry-after-ms", "120")
    .times(1);
mock.on_system_one().respond(answers).times(1);
```

`mock.journal()` records method, path, headers, JSON body, and timestamps. Use it to assert `X-TypeSafe-Retry-Count` and attempt order.

Run the bundled example (no live key):

```bash
cargo run -p typesafe-rs --example quickstart
```

## Blocking client

```toml
typesafe-rs = { version = "0.1", features = ["blocking"] }
```

```rust
use typesafe_rs::{questions, BlockingClient, Question};

let client = BlockingClient::from_env()?;
let response = client.system_one(
    "Help! My payouts have been failing for 3 days.",
    questions! { "urgent" => Question::noul("Does this convey urgency?") },
)?;
```

Do not construct `BlockingClient` inside an existing Tokio runtime (`block_on` will panic).

## Crates

| Crate | Role |
|---|---|
| [`typesafe-rs`](https://docs.rs/typesafe-rs) | Async `Client`, optional `BlockingClient`, wire types, retry, errors. `#![forbid(unsafe_code)]` |
| [`typesafe-rs-mock`](https://docs.rs/typesafe-rs-mock) | In-process mock for tests |

`Backend` is implemented for `Client` so callers can depend on the trait rather than the HTTP type.

## Feature flags

| Feature | Default | Notes |
|---|---|---|
| `rustls` | yes | TLS via rustls (platform verifier) |
| `native-tls` | no | Platform TLS instead |
| `tracing` | yes | `typesafe.request` span and `retry_scheduled` events |
| `blocking` | no | `BlockingClient` |

## Conformance

JSON fixtures in [`conformance/fixtures/`](./conformance) drive the real client against the real mock. Coverage includes 429 + `retry-after-ms`, non-retryable 400, env precedence, unknown answer types, missing `models` array, and retry-count headers.

```bash
cargo test -p typesafe-rs --test conformance
```

## Development

```bash
cargo test --workspace --all-features
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo run -p typesafe-rs --example triage
```

See [CONTRIBUTING.md](./CONTRIBUTING.md). Behaviour marked **[parity]** in [SPEC.md](./SPEC.md) must match the official Python and TypeScript SDKs.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT](LICENSE) at your option.
