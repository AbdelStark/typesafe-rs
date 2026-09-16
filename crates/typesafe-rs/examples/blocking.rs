//! Blocking client against `typesafe-rs-mock`.
//!
//! ```text
//! cargo run -p typesafe-rs --example blocking --features blocking
//! ```

use serde_json::json;
use typesafe_rs::{ClientConfig, Question, questions};
use typesafe_rs_mock::{MockServer, noul};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let mock = rt.block_on(MockServer::start());
    mock.on_system_one().respond(json!({
        "urgent": noul(0.91),
    }));

    let client = ClientConfig::new()
        .api_key("test")
        .base_url(mock.url())
        .build_blocking()?;

    let response = client.system_one(
        "Help! My payouts have been failing for 3 days.",
        questions! { "urgent" => Question::noul("Does this convey urgency?") },
    )?;
    println!("urgent noul = {:?}", response.noul("urgent"));

    drop(client);
    drop(mock);
    drop(rt);
    Ok(())
}
