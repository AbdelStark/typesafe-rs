# typesafe-rs-mock

In-process HTTP mock for [`typesafe-rs`](https://crates.io/crates/typesafe-rs).

Binds `127.0.0.1:0`, speaks HTTP/1.1, and scripts `POST /v1/systemone` and `GET /v1/models`. Tests use a real `Client` over loopback.

```toml
[dev-dependencies]
typesafe-rs-mock = "0.1"
```

```rust
use typesafe_rs::{questions, Client, ClientConfig, Question};
use typesafe_rs_mock::{noul, MockServer};

let mock = MockServer::start().await;
mock.on_system_one().respond(serde_json::json!({
    "urgent": noul(0.97),
}));

let client = Client::new(
    ClientConfig::new()
        .api_key("test")
        .base_url(mock.url()),
)?;

let response = client
    .system_one("ticket text", questions! {
        "urgent" => Question::noul("Does this convey urgency?"),
    })
    .await?;

assert_eq!(response.noul("urgent"), Some(0.97));
```

Script sequential failures, inject latency, and inspect `mock.journal()` for headers and bodies. See the [typesafe-rs README](https://github.com/AbdelStark/typesafe-rs#testing).

Licensed under MIT OR Apache-2.0.
