# Implementation notes (v0.1)

## Architecture

```
Call (system_one / models.list)
        │
        ▼
   ClientConfig  ── overlay env (code > TYPESAFE_* > defaults)
        │
        ▼
   validate_questions (Choice 2..=255, Score >= 2, keys)
        │
        ▼
   Retry loop (RetryPolicy)
        │   delay_after_failure(attempt, headers)
        │   honor retry-after-ms, then Retry-After, else jittered backoff
        ▼
   reqwest (rustls, HTTP/2)  ── headers every attempt
        │                         Authorization, Accept, User-Agent,
        │                         X-TypeSafe-SDK, X-TypeSafe-Runtime
        │                         X-TypeSafe-Retry-Count on retries only
        ▼
   decode JSON → SystemOneResponse / Vec<ModelCard>
        │         unknown answer types → Answer::Unknown
        │         missing models array → Error::UnexpectedShape
        ▼
   ResponseMeta (request_id, status, headers, attempts)
```

Public surface:

| Item | Role |
|---|---|
| `Client` | Async, `Arc` inner, `Send + Sync`, cheap `Clone` |
| `ClientConfig::build` | Preferred constructor; `build_blocking` with feature `blocking` |
| `Result<T>` | Alias for `std::result::Result<T, Error>` |
| `BlockingClient` | Feature `blocking`; current-thread Tokio runtime |
| `Backend` | `name()` + `system_one(&request, &opts)`; implemented for `Client` |
| `Question::{noul,choice,score}` | Builders: `.when_true/.when_false`, `.option`, `.level` |
| `questions! { "k" => Question::noul("...") }` | Insertion-ordered `IndexMap` |
| `RetryPolicy::{default,none,conservative}` | Official defaults; conservative = 408/429 + pre-send connect, no 5xx/timeouts |
| `CallOptions` | Per-call timeout, retry, headers, model |

Protected headers (`Authorization`, `Accept`, `User-Agent`, `X-TypeSafe-SDK`, `X-TypeSafe-Runtime`, `X-TypeSafe-Retry-Count`) cannot be overridden via `default_headers` or `CallOptions`.

## Mock + conformance

`typesafe-rs-mock::MockServer::start()` binds `127.0.0.1:0` with axum (HTTP/1.1).

```rust
let mock = MockServer::start().await;
mock.on_system_one()
    .respond_status(429)
    .header("retry-after-ms", "120")
    .times(1);
mock.on_system_one().respond(answers_map).times(1);
let client = Client::new(ClientConfig {
    api_key: Some("test".into()),
    base_url: Some(mock.url()),
    ..ClientConfig::default()
})?;
```

`mock.journal()` records method, path, headers, JSON body, and timestamps. Tests assert retry-count headers and delays from the journal, not from a fake client.

`crates/typesafe-rs/tests/conformance.rs` loads `conformance/fixtures/*.json` and runs each script through that stack.

## Features

| Feature | Default | Notes |
|---|---|---|
| `rustls` | yes | `reqwest/rustls` |
| `native-tls` | no | Platform TLS instead |
| `tracing` | yes | `typesafe.request` span, `retry_scheduled` event |
| `blocking` | no | `BlockingClient` |

## Errors

`Error` is non-exhaustive. `Display` redacts `Bearer` tokens and never dumps request bodies. Raw error JSON is stored on `ApiError.body` for inspection.
