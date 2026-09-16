//! Noul, Choice, and Score against `typesafe-rs-mock`.
//!
//! ```text
//! cargo run -p typesafe-rs --example triage
//! ```

use serde_json::json;
use typesafe_rs::{ClientConfig, Question, questions};
use typesafe_rs_mock::{MockServer, choice, noul, score};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mock = MockServer::start().await;
    mock.on_system_one().respond(json!({
        "urgent": noul(0.97),
        "team": choice("technical", 0.82),
        "frustration": score(1.6, 0.78),
    }));

    let client = ClientConfig::new()
        .api_key("test")
        .base_url(mock.url())
        .build()?;

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
    println!(
        "team        = {:?}",
        response.choice("team").map(|c| c.choice)
    );
    println!(
        "frustration = {:?}",
        response.score("frustration").map(|s| s.score)
    );
    Ok(())
}
