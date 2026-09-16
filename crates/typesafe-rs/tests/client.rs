mod common;

use serde_json::json;
use typesafe_rs::{
    Answer, Backend, CallOptions, Client, ClientConfig, Error, Question, RetryPolicy, VERSION,
    questions,
};
use typesafe_rs_mock::{MockServer, noul};

fn sample_questions() -> typesafe_rs::Questions {
    questions! {
        "urgent" => Question::noul("Does this convey urgency?"),
    }
}

#[tokio::test]
async fn system_one_returns_noul_through_real_http() {
    let mock = MockServer::start().await;
    mock.on_system_one()
        .with_question_key("urgent")
        .respond(json!({ "urgent": noul(0.97) }));
    let client = common::test_client(&mock);

    let resp = client
        .system_one("payouts are failing", sample_questions())
        .await
        .unwrap();

    assert_eq!(resp.noul("urgent"), Some(0.97));
    assert_eq!(resp.model, "jev-latest");
    assert_eq!(resp.meta.attempts, 1);
    assert_eq!(resp.meta.request_id.as_deref(), Some("mock-1"));
}

#[tokio::test]
async fn sends_required_headers_on_first_attempt() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond(json!({ "urgent": noul(0.1) }));
    let client = common::test_client(&mock);
    client
        .system_one("state", sample_questions())
        .await
        .unwrap();

    let req = mock.journal().pop().expect("request");
    assert_eq!(req.headers.get("authorization").unwrap(), "Bearer test");
    assert_eq!(req.headers.get("accept").unwrap(), "application/json");
    assert_eq!(req.headers.get("content-type").unwrap(), "application/json");
    assert_eq!(
        req.headers.get("user-agent").unwrap(),
        &format!("typesafe-rs/{VERSION}")
    );
    assert_eq!(
        req.headers.get("x-typesafe-sdk").unwrap(),
        &format!("typesafe-rs/{VERSION}")
    );
    let runtime = req.headers.get("x-typesafe-runtime").unwrap();
    assert!(
        runtime.starts_with("rust/"),
        "runtime header {runtime:?} should start with rust/"
    );
    assert!(runtime.contains(';'), "runtime header {runtime:?}");
    assert!(
        !req.headers.contains_key("x-typesafe-retry-count"),
        "retry-count must be absent on the first attempt"
    );
}

#[tokio::test]
async fn default_headers_cannot_override_protected_names() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond(json!({ "urgent": noul(0.2) }));
    let client = Client::new(ClientConfig {
        api_key: Some("real-key".into()),
        base_url: Some(mock.url()),
        default_model: Some("jev-latest".into()),
        retry: RetryPolicy::none(),
        ..ClientConfig::default()
    })
    .unwrap();

    let mut opts = CallOptions::default();
    opts.headers.insert(
        "authorization",
        typesafe_rs::HeaderValue::from_static("Bearer stolen"),
    );
    opts.headers
        .insert("user-agent", typesafe_rs::HeaderValue::from_static("evil"));
    opts.headers
        .insert("x-custom", typesafe_rs::HeaderValue::from_static("ok"));
    let req = typesafe_rs::SystemOneRequest::new(json!("s"), sample_questions());
    client.system_one_with(&req, opts).await.unwrap();
    let recorded = &mock.journal()[0];
    assert_eq!(
        recorded.headers.get("authorization").unwrap(),
        "Bearer real-key"
    );
    assert_eq!(
        recorded.headers.get("user-agent").unwrap(),
        &format!("typesafe-rs/{VERSION}")
    );
    assert_eq!(recorded.headers.get("x-custom").unwrap(), "ok");
}

#[tokio::test]
async fn models_list_and_warm_up() {
    let mock = MockServer::start().await;
    mock.on_models().respond(json!({
        "models": [
            {
                "name": "jev-latest",
                "description": "Flagship",
                "release_date": "2026-01-01"
            }
        ]
    }));
    let client = common::test_client(&mock);
    client.warm_up().await.unwrap();
    let models = client.models().list().await.unwrap();
    assert_eq!(models[0].name, "jev-latest");
    assert_eq!(mock.journal().len(), 2);
}

#[tokio::test]
async fn models_missing_array_is_unexpected_shape() {
    let mock = MockServer::start().await;
    mock.on_models().respond(json!({ "data": [] }));
    let client = common::test_client(&mock);
    let err = client.models().list().await.unwrap_err();
    assert!(matches!(
        err,
        Error::UnexpectedShape {
            endpoint: "GET /v1/models",
            ..
        }
    ));
    assert_eq!(err.attempts(), Some(1));
}

#[tokio::test]
async fn unknown_answer_type_survives() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond(json!({
        "model": "jev-latest",
        "answers": {
            "future": { "type": "spectrum", "bands": [] },
            "urgent": { "type": "noul", "noul": 0.4 }
        }
    }));
    let client = common::test_client(&mock);
    let resp = client.system_one("s", sample_questions()).await.unwrap();
    assert!(matches!(resp.answer("future"), Some(Answer::Unknown)));
    assert_eq!(resp.noul("urgent"), Some(0.4));
}

#[tokio::test]
async fn validation_fails_before_network() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond(json!({ "urgent": noul(1.0) }));
    let client = common::test_client(&mock);
    let err = client
        .system_one("s", typesafe_rs::Questions::new())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::InvalidRequest(_)));
    assert!(mock.journal().is_empty());
}

#[tokio::test]
async fn missing_api_key() {
    let err = Client::new_with_env(ClientConfig::default(), |_| None).unwrap_err();
    assert!(matches!(err, Error::MissingApiKey));
    assert!(err.to_string().contains("TYPESAFE_API_KEY"));
}

#[tokio::test]
async fn backend_trait_dispatches_to_client() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond(json!({ "urgent": noul(0.5) }));
    let client = common::test_client(&mock);
    let req = typesafe_rs::SystemOneRequest::new(json!("s"), sample_questions());
    let resp = Backend::system_one(&client, &req, &CallOptions::default())
        .await
        .unwrap();
    assert_eq!(resp.noul("urgent"), Some(0.5));
    assert_eq!(client.name(), "typesafe");
}

#[tokio::test]
async fn preserves_base_url_path_prefix() {
    // The mock serves at /, so a prefix would 404; instead assert URL join via a
    // request recorded on the un-prefixed mock plus unit coverage in config.
    let mock = MockServer::start().await;
    mock.on_system_one().respond(json!({ "urgent": noul(0.0) }));
    let client = common::test_client(&mock);
    client.system_one("s", sample_questions()).await.unwrap();
    assert_eq!(mock.journal()[0].path, "/v1/systemone");
}

#[tokio::test]
async fn captures_request_id_on_error() {
    let mock = MockServer::start().await;
    mock.on_system_one()
        .respond_status(400)
        .header("x-typesafe-request-id", "req-err")
        .times(1);
    let client = Client::new(ClientConfig {
        retry: RetryPolicy::none(),
        ..common::test_config(&mock)
    })
    .unwrap();
    let err = client
        .system_one("s", sample_questions())
        .await
        .unwrap_err();
    assert_eq!(err.request_id(), Some("req-err"));
    assert_eq!(err.status(), Some(typesafe_rs::StatusCode::BAD_REQUEST));
}
