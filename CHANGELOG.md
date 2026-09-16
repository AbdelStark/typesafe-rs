# Changelog

## Unreleased

- `Result` type alias
- `ClientConfig::build` / `build_blocking`, `header` builders, `CallOptions::header`
- `Error::as_api`, `kind`, `is_rate_limited`, `is_auth`, `is_timeout`, `is_connection`
- `BlockingClient::base_url` and `BlockingModels::list_with`
- Examples: `triage`, `models`, `blocking`
- Conformance fixtures for 500 retry, `Retry-After` seconds, 401, and successful `GET /v1/models`
- CI: fmt, clippy, tests, rustdoc, MSRV 1.85

## 0.1.0 — 2026-09-16

First release of the Rust client for TypeSafe's System One API.

- Async `Client` and optional blocking `BlockingClient`
- `POST /v1/systemone` and `GET /v1/models`
- Env vars, defaults, retries, and identification headers matching the official Python and TypeScript SDKs
- `Backend` trait (`impl Backend for Client`)
- `typesafe-rs-mock` in-process server and JSON conformance fixtures
- Dual license: MIT OR Apache-2.0

This crate is **not** an official TypeSafe product.
