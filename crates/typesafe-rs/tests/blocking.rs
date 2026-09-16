use serde_json::json;
use typesafe_rs::{BlockingClient, ClientConfig, Question, RetryPolicy, questions};
use typesafe_rs_mock::{MockServer, noul};

#[test]
fn blocking_system_one_and_models() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .unwrap();
    let mock = rt.block_on(MockServer::start());
    mock.on_system_one()
        .respond(json!({ "urgent": noul(0.88) }));
    mock.on_models().respond(json!({
        "models": [{
            "name": "jev-latest",
            "description": "Flagship",
            "release_date": "2026-01-01"
        }]
    }));

    let client = BlockingClient::new(ClientConfig {
        api_key: Some("test".into()),
        base_url: Some(mock.url()),
        default_model: Some("jev-latest".into()),
        retry: RetryPolicy::none(),
        ..ClientConfig::default()
    })
    .unwrap();

    let resp = client
        .system_one("s", questions! { "urgent" => Question::noul("urgent?") })
        .unwrap();
    assert_eq!(resp.noul("urgent"), Some(0.88));

    let models = client.models().list().unwrap();
    assert_eq!(models[0].name, "jev-latest");
    client.warm_up().unwrap();

    drop(client);
    drop(mock);
    drop(rt);
}
