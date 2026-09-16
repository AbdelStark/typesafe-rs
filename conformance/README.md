# Conformance fixtures

JSON scripts under `fixtures/` drive the **real** `typesafe-rs` `Client` against
the **real** `typesafe-rs-mock` HTTP server. The runner is
`crates/typesafe-rs/tests/conformance.rs` (`cargo test -p typesafe-rs --test conformance`).

Each fixture describes:

- optional env-var fallbacks (`TYPESAFE_API_KEY`, `TYPESAFE_BASE_URL`, `TYPESAFE_DEFAULT_MODEL`)
- client config (retry policy)
- a scripted sequence of HTTP responses
- expectations: success/error, attempt count, `X-TypeSafe-Retry-Count`, delays

v0.1 coverage:

| Fixture | Claim |
|---|---|
| `retry-429-retry-after-ms.json` | 429 + `retry-after-ms` is honored; second attempt sends `X-TypeSafe-Retry-Count: 1` |
| `non-retryable-400.json` | 400 is not retried |
| `env-precedence.json` | explicit config wins over env; unset fields read env |
| `unknown-answer-type.json` | unknown answer `type` does not fail deserialize |
| `models-missing-array.json` | `GET /v1/models` without a `models` array → `Error::UnexpectedShape` |
| `header-retry-count.json` | retry-count header is omitted on the first attempt |

## Official SDK parity (later)

A TypeScript runner that executes the same fixtures against `@typesafe-ai/sdk`
is **not** in v0.1. The intended layout is `conformance/ts/run.mjs` using a local
HTTP server, run weekly against the latest npm version. Offer this fixture format
to TypeSafe as a shared cross-SDK suite when the crate is public.
