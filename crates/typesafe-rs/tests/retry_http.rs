//! Retry tests that go through the real HTTP client and mock server.

mod common;

use std::time::Duration;

use serde_json::json;
use typesafe_rs::{
    ApiErrorKind, CallOptions, Client, ClientConfig, Error, Question, RetryPolicy, questions,
};
use typesafe_rs_mock::{MockServer, noul};

fn qs() -> typesafe_rs::Questions {
    questions! { "urgent" => Question::noul("urgent?") }
}

#[tokio::test]
async fn retries_429_honoring_retry_after_ms() {
    let mock = MockServer::start().await;
    mock.on_system_one()
        .respond_status(429)
        .header("retry-after-ms", "120")
        .times(1);
    mock.on_system_one()
        .respond(json!({ "urgent": noul(0.9) }))
        .times(1);

    // Default policy: backoff is 500ms. If retry-after-ms is ignored, delay ≈ 500ms.
    let client = common::default_retry_client(&mock);
    let resp = client.system_one("s", qs()).await.unwrap();
    assert_eq!(resp.noul("urgent"), Some(0.9));
    assert_eq!(resp.meta.attempts, 2);

    let journal = mock.journal();
    assert_eq!(journal.len(), 2);
    assert!(
        !journal[0].headers.contains_key("x-typesafe-retry-count"),
        "first attempt must omit retry-count"
    );
    assert_eq!(
        journal[1]
            .headers
            .get("x-typesafe-retry-count")
            .map(String::as_str),
        Some("1")
    );

    let gap = mock.request_gaps()[0];
    assert!(
        gap >= Duration::from_millis(90),
        "expected wait for retry-after-ms=120, got {gap:?}"
    );
    assert!(
        gap < Duration::from_millis(350),
        "delay {gap:?} looks like 500ms backoff, not retry-after-ms=120"
    );
}

#[tokio::test]
async fn non_retryable_400_is_not_retried() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond_status(400).times(1);
    mock.on_system_one()
        .respond(json!({ "urgent": noul(1.0) }))
        .times(1);

    let client = common::test_client(&mock);
    let err = client.system_one("s", qs()).await.unwrap_err();
    match err {
        Error::Api(api) => {
            assert_eq!(api.kind, ApiErrorKind::BadRequest);
            assert_eq!(api.attempts, 1);
        }
        other => panic!("expected Api error, got {other:?}"),
    }
    assert_eq!(mock.journal().len(), 1);
}

#[tokio::test]
async fn retries_500_then_succeeds() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond_status(500).times(1);
    mock.on_system_one()
        .respond(json!({ "urgent": noul(0.3) }))
        .times(1);
    let client = common::test_client(&mock);
    let resp = client.system_one("s", qs()).await.unwrap();
    assert_eq!(resp.noul("urgent"), Some(0.3));
    assert_eq!(mock.journal().len(), 2);
}

#[tokio::test]
async fn conservative_does_not_retry_5xx() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond_status(500).times(1);
    mock.on_system_one()
        .respond(json!({ "urgent": noul(0.3) }))
        .times(1);
    let client = Client::new(ClientConfig {
        retry: RetryPolicy::conservative(),
        ..common::test_config(&mock)
    })
    .unwrap();
    let err = client.system_one("s", qs()).await.unwrap_err();
    assert!(matches!(err, Error::Api(_)));
    assert_eq!(mock.journal().len(), 1);
}

#[tokio::test]
async fn none_does_not_retry_429() {
    let mock = MockServer::start().await;
    mock.on_system_one()
        .respond_status(429)
        .header("retry-after-ms", "1")
        .times(1);
    mock.on_system_one()
        .respond(json!({ "urgent": noul(0.3) }))
        .times(1);
    let client = Client::new(ClientConfig {
        retry: RetryPolicy::none(),
        ..common::test_config(&mock)
    })
    .unwrap();
    let err = client.system_one("s", qs()).await.unwrap_err();
    match err {
        Error::Api(api) => assert_eq!(api.kind, ApiErrorKind::RateLimit),
        other => panic!("{other:?}"),
    }
    assert_eq!(mock.journal().len(), 1);
}

#[tokio::test]
async fn retries_timeouts_then_errors() {
    let mock = MockServer::start().await;
    mock.on_system_one()
        .delay(Duration::from_millis(200))
        .respond(json!({ "urgent": noul(1.0) }));

    let policy = RetryPolicy {
        max_retries: 1,
        backoff_initial: Duration::from_millis(5),
        backoff_max: Duration::from_millis(5),
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    };
    let client = Client::new(ClientConfig {
        timeout: Duration::from_millis(40),
        retry: policy.clone(),
        ..common::test_config(&mock)
    })
    .unwrap();

    let err = client.system_one("s", qs()).await.unwrap_err();
    assert!(matches!(err, Error::Timeout { .. }), "{err:?}");
    assert_eq!(mock.journal().len(), 2);
}

#[tokio::test]
async fn conservative_does_not_retry_timeouts() {
    let mock = MockServer::start().await;
    mock.on_system_one()
        .delay(Duration::from_millis(200))
        .respond(json!({ "urgent": noul(1.0) }));
    let client = Client::new(ClientConfig {
        timeout: Duration::from_millis(40),
        retry: RetryPolicy::conservative(),
        ..common::test_config(&mock)
    })
    .unwrap();
    let err = client.system_one("s", qs()).await.unwrap_err();
    assert!(matches!(err, Error::Timeout { .. }), "{err:?}");
    assert_eq!(mock.journal().len(), 1);
}

#[tokio::test]
async fn retries_connection_errors() {
    let policy = RetryPolicy {
        max_retries: 1,
        backoff_initial: Duration::from_millis(5),
        backoff_max: Duration::from_millis(5),
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    };
    let client = Client::new(ClientConfig {
        api_key: Some("test".into()),
        base_url: Some("http://127.0.0.1:1".parse().unwrap()),
        default_model: Some("jev-latest".into()),
        timeout: Duration::from_millis(200),
        retry: policy,
        ..ClientConfig::default()
    })
    .unwrap();

    let started = std::time::Instant::now();
    let err = client.system_one("s", qs()).await.unwrap_err();
    assert!(matches!(err, Error::Connection(_)), "{err:?}");
    assert!(
        started.elapsed() >= Duration::from_millis(5),
        "should have backed off between attempts"
    );
}

#[tokio::test]
async fn per_call_retry_override() {
    let mock = MockServer::start().await;
    mock.on_system_one().respond_status(429).times(2);
    mock.on_system_one()
        .respond(json!({ "urgent": noul(0.1) }))
        .times(1);
    let client = Client::new(ClientConfig {
        retry: RetryPolicy::none(),
        ..common::test_config(&mock)
    })
    .unwrap();
    let req = typesafe_rs::SystemOneRequest::new(json!("s"), qs());
    let err = client
        .system_one_with(&req, CallOptions::default())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Api(_)));
    assert_eq!(mock.journal().len(), 1);

    let resp = client
        .system_one_with(
            &req,
            CallOptions::new().retry(RetryPolicy {
                backoff_initial: Duration::from_millis(5),
                backoff_jitter: 0.0,
                ..RetryPolicy::default()
            }),
        )
        .await
        .unwrap();
    assert_eq!(resp.noul("urgent"), Some(0.1));
    assert_eq!(mock.journal().len(), 3);
}

#[tokio::test]
async fn delay_uses_retry_policy_not_ad_hoc_math() {
    // Guard against tests (and callers) inventing their own backoff formula.
    let policy = RetryPolicy::default();
    let via_policy = policy.delay_after_failure_with(0, None, || 0.0);
    assert_eq!(via_policy, policy.backoff_initial);
}
