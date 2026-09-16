# Changelog

## 0.1.0 — 2026-09-16

Initial MVP of the community Rust SDK for TypeSafe's System One API.

- Async `Client` and optional blocking `BlockingClient`
- `POST /v1/systemone` and `GET /v1/models`
- Official SDK parity for env vars, defaults, retries, and identification headers
- `Backend` trait (`impl Backend for Client`)
- `typesafe-rs-mock` in-process server and JSON conformance fixtures
- Dual license: MIT OR Apache-2.0

This crate is **not** an official TypeSafe product.
