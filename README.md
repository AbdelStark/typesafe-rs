# typesafe-rs

Community Rust SDK for [TypeSafe](https://typesafe.ai)'s System One API.

**This is not an official TypeSafe product.** The repository is private until launch.

Behaviour aims at parity with the official Python SDK and [`@typesafe-ai/sdk`](https://www.npmjs.com/package/@typesafe-ai/sdk) 0.6.0 (retries, env vars, headers, error shapes). See [PRD](./PRD.md) and [SPEC](./SPEC.md).

## Quick start (mock)

No live API key required:

```bash
cargo run -p typesafe-rs --example quickstart
```

```rust
use typesafe_rs::{questions, Client, ClientConfig, Question};
use typesafe_rs_mock::{noul, MockServer};

# async fn demo() -> Result<(), typesafe_rs::Error> {
let mock = MockServer::start().await;
mock.on_system_one().respond(serde_json::json!({
    "urgent": noul(0.97),
}));

let client = Client::new(ClientConfig {
    api_key: Some("test".into()),
    base_url: Some(mock.url()),
    ..ClientConfig::default()
})?;

let response = client
    .system_one(
        "Help! My payouts have been failing for 3 days.",
        questions! {
            "urgent" => Question::noul("Does this convey urgency?"),
        },
    )
    .await?;

assert_eq!(response.noul("urgent"), Some(0.97));
# Ok(())
# }
```

Against the live API, set `TYPESAFE_API_KEY` and use `Client::from_env()`. Default base URL is `https://api.typesafe.ai`, default model `jev-latest`, default timeout 10s.

## Mock server

`typesafe-rs-mock` is an in-process HTTP server for tests:

- scripted `POST /v1/systemone` and `GET /v1/models` responses
- sequential `429` then `200`, including `retry-after-ms`
- request journal (headers, body, timestamps) for retry-count assertions

```rust
mock.on_system_one()
    .respond_status(429)
    .header("retry-after-ms", "120")
    .times(1);
mock.on_system_one().respond(answers).times(1);
```

## Comparison with `typesafe-ai` 0.1.0 (16 Sep 2026)

Joey ([Twister915](https://github.com/Twister915)) published [`typesafe-ai`](https://crates.io/crates/typesafe-ai) 0.1.0 the same day this crate was specified. That crate is carefully built and deliberately minimal. typesafe-rs exists because s1-rs / Reflex / Sieve need **official SDK retry and config semantics** plus a mock that those crates can share.

| Behaviour (16 Sep 2026) | Official Python/TS SDKs | `typesafe-ai` 0.1.0 | `typesafe-rs` 0.1.0 |
|---|---|---|---|
| Retryable statuses | 408, 429, all 5xx | 429, 529 | 408, 429, all 5xx |
| Connection errors and timeouts retried | Yes | No | Yes |
| Default timeout | 10 s | 60 s per attempt | 10 s |
| Backoff | 500 ms, 5 s cap, 25% jitter; `retry-after-ms` / `Retry-After` up to 60 s | 250 ms, 8 s cap; server delay up to 60 s | 500 ms, 5 s cap, 25% jitter; same headers |
| `GET /v1/models` | Yes | Not covered | Yes |
| `TYPESAFE_API_KEY` / `BASE_URL` / `DEFAULT_MODEL` | Yes | Not covered | Yes |
| `X-TypeSafe-SDK` / `Runtime` / `Retry-Count` | Yes | Not covered | Yes |

Official behaviour was read from `@typesafe-ai/sdk` 0.6.0 and the Python SDK docs on 16 Sep 2026. Re-verify before treating this table as current.

Credit: [typesafe-ai](https://github.com/Twister915/typesafe-ai) by Joey / Twister915.

## Crates

- `typesafe-rs` — async + blocking client (`#![forbid(unsafe_code)]`)
- `typesafe-rs-mock` — in-process mock HTTP server

Typed derive (enums → questions) lives in **s1-rs**, not here.

## License

MIT OR Apache-2.0.
