//! Local quickstart against `typesafe-rs-mock`.
//!
//! ```text
//! cargo run -p typesafe-rs --example quickstart
//! ```
//!
//! This example does **not** call the live TypeSafe API.

use serde_json::json;
use typesafe_rs::{Client, ClientConfig, Question, questions};
use typesafe_rs_mock::{MockServer, noul};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mock = MockServer::start().await;
    mock.on_system_one().respond(json!({
        "urgent": noul(0.97),
    }));

    let client = Client::new(ClientConfig {
        api_key: Some("test".into()),
        base_url: Some(mock.url()),
        default_model: Some("jev-latest".into()),
        ..ClientConfig::default()
    })?;

    let response = client
        .system_one(
            "Help! My payouts have been failing for 3 days.",
            questions! {
                "urgent" => Question::noul("Does this convey urgency?")
                    .when_true("Explicitly time-sensitive")
                    .when_false("No time pressure"),
            },
        )
        .await?;

    println!("urgent noul = {:?}", response.noul("urgent"));
    println!("request id  = {:?}", response.meta.request_id);
    Ok(())
}
