//! List models against `typesafe-rs-mock`.
//!
//! ```text
//! cargo run -p typesafe-rs --example models
//! ```

use serde_json::json;
use typesafe_rs::ClientConfig;
use typesafe_rs_mock::MockServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mock = MockServer::start().await;
    mock.on_models().respond(json!({
        "models": [{
            "name": "jev-latest",
            "description": "TypeSafe flagship",
            "release_date": "2026-01-01"
        }]
    }));

    let client = ClientConfig::new()
        .api_key("test")
        .base_url(mock.url())
        .build()?;

    client.warm_up().await?;
    for model in client.models().list().await? {
        println!("{} — {}", model.name, model.description);
    }
    Ok(())
}
