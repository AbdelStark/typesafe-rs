//! Conformance runner: real `Client` against the real mock server, driven by JSON fixtures.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use typesafe_rs::{
    Answer, ApiErrorKind, Client, ClientConfig, Error, Question, RetryPolicy, questions,
};
use typesafe_rs_mock::MockServer;

#[derive(Debug, Deserialize)]
struct Fixture {
    id: String,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    config: FixtureConfig,
    #[serde(default)]
    endpoint: Endpoint,
    script: Vec<ScriptStep>,
    expect: Expect,
}

#[derive(Debug, Default, Deserialize)]
struct FixtureConfig {
    api_key: Option<String>,
    #[serde(default)]
    retry: FixtureRetry,
}

#[derive(Debug, Default, Deserialize)]
struct FixtureRetry {
    max_retries: Option<u32>,
    backoff_initial_ms: Option<u64>,
    backoff_jitter: Option<f64>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Endpoint {
    #[default]
    SystemOne,
    Models,
}

#[derive(Debug, Deserialize)]
struct ScriptStep {
    status: u16,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    body: Option<Value>,
    body_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Expect {
    result: String,
    #[serde(default)]
    attempts: Option<u32>,
    #[serde(default)]
    delays_ms: Option<Vec<u64>>,
    #[serde(default)]
    retry_count_headers: Option<Vec<Option<String>>>,
    #[serde(default)]
    error_kind: Option<String>,
    #[serde(default)]
    authorization: Option<String>,
    #[serde(default)]
    request_model: Option<String>,
    #[serde(default)]
    unknown_answer_key: Option<String>,
    #[serde(default)]
    noul: Option<BTreeMap<String, f64>>,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures")
}

fn load_fixtures() -> Vec<(PathBuf, Fixture)> {
    let dir = fixtures_dir();
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).expect("conformance/fixtures") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        if value.get("script").is_none() {
            continue;
        }
        let fixture: Fixture = serde_json::from_value(value)
            .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        out.push((path, fixture));
    }
    out.sort_by(|a, b| a.1.id.cmp(&b.1.id));
    out
}

fn load_body(dir: &Path, step: &ScriptStep) -> Option<Value> {
    if let Some(body) = &step.body {
        return Some(body.clone());
    }
    if let Some(file) = &step.body_file {
        let text = fs::read_to_string(dir.join(file)).unwrap();
        return Some(serde_json::from_str(&text).unwrap());
    }
    None
}

fn retry_from_fixture(spec: &FixtureRetry) -> RetryPolicy {
    let mut retry = RetryPolicy::default();
    if let Some(max) = spec.max_retries {
        retry.max_retries = max;
    }
    if let Some(ms) = spec.backoff_initial_ms {
        retry.backoff_initial = Duration::from_millis(ms);
        retry.backoff_max = Duration::from_millis(ms.max(20));
    }
    if let Some(j) = spec.backoff_jitter {
        retry.backoff_jitter = j;
    }
    retry
}

#[tokio::test]
async fn conformance_fixtures() {
    let dir = fixtures_dir();
    let fixtures = load_fixtures();
    assert!(
        !fixtures.is_empty(),
        "expected conformance fixtures in {}",
        dir.display()
    );

    for (path, fixture) in fixtures {
        run_fixture(&dir, &fixture)
            .await
            .unwrap_or_else(|e| panic!("{} ({}): {e}", fixture.id, path.display()));
    }
}

async fn run_fixture(dir: &Path, fixture: &Fixture) -> Result<(), String> {
    let mock = MockServer::start().await;
    for step in &fixture.script {
        match fixture.endpoint {
            Endpoint::SystemOne => {
                let mut stub = mock.on_system_one().respond_status(step.status);
                if let Some(body) = load_body(dir, step) {
                    stub = stub.respond_raw(body);
                }
                for (k, v) in &step.headers {
                    stub = stub.header(k, v);
                }
                let _ = stub.times(1);
            }
            Endpoint::Models => {
                let mut stub = mock.on_models().respond_status(step.status);
                if let Some(body) = load_body(dir, step) {
                    stub = stub.respond_raw(body);
                }
                for (k, v) in &step.headers {
                    stub = stub.header(k, v);
                }
                let _ = stub.times(1);
            }
        }
    }

    let mut cfg = ClientConfig {
        base_url: Some(mock.url()),
        retry: retry_from_fixture(&fixture.config.retry),
        timeout: Duration::from_secs(5),
        ..ClientConfig::default()
    };
    if let Some(key) = &fixture.config.api_key {
        cfg.api_key = Some(key.as_str().into());
    } else if !fixture.env.contains_key("TYPESAFE_API_KEY") {
        cfg.api_key = Some("test".into());
    }

    let env = fixture.env.clone();
    let client =
        Client::new_with_env(cfg, |k| env.get(k).cloned()).map_err(|e| format!("client: {e}"))?;

    let qs = questions! { "urgent" => Question::noul("Does this convey urgency?") };

    match fixture.endpoint {
        Endpoint::SystemOne => {
            let result = client.system_one("conformance state", qs).await;
            check_expect(
                fixture,
                &mock,
                result.map(|r| Outcome::SystemOne(Box::new(r))),
            )?;
        }
        Endpoint::Models => {
            let result = client.models().list().await;
            check_expect(fixture, &mock, result.map(|_| Outcome::Models))?;
        }
    }
    Ok(())
}

enum Outcome {
    SystemOne(Box<typesafe_rs::SystemOneResponse>),
    Models,
}

fn check_expect(
    fixture: &Fixture,
    mock: &MockServer,
    result: Result<Outcome, Error>,
) -> Result<(), String> {
    let journal = mock.journal();

    if let Some(expected) = fixture.expect.attempts {
        if journal.len() as u32 != expected {
            return Err(format!(
                "attempts: journal {} != expected {expected}",
                journal.len()
            ));
        }
    }

    if let Some(expected) = &fixture.expect.retry_count_headers {
        if journal.len() != expected.len() {
            return Err(format!(
                "retry_count_headers length {} != journal {}",
                expected.len(),
                journal.len()
            ));
        }
        for (i, exp) in expected.iter().enumerate() {
            let got = journal[i].headers.get("x-typesafe-retry-count").cloned();
            if got != *exp {
                return Err(format!(
                    "retry-count attempt {i}: got {got:?} expected {exp:?}"
                ));
            }
        }
    }

    if let Some(expected_delays) = &fixture.expect.delays_ms {
        let gaps = mock.request_gaps();
        if gaps.len() < expected_delays.len() {
            return Err("not enough request gaps for delays_ms".into());
        }
        for (i, &want) in expected_delays.iter().enumerate() {
            let got = gaps[i];
            let want_d = Duration::from_millis(want);
            if got < want_d.saturating_sub(Duration::from_millis(30)) {
                return Err(format!("delay[{i}] {got:?} shorter than {want}ms"));
            }
            let upper = want_d + Duration::from_millis(200);
            if got > upper {
                return Err(format!(
                    "delay[{i}] {got:?} longer than {:?}; retry-after may have been ignored",
                    upper
                ));
            }
        }
    }

    match (fixture.expect.result.as_str(), result) {
        ("ok", Ok(outcome)) => {
            if let Some(auth) = &fixture.expect.authorization {
                let got = journal[0]
                    .headers
                    .get("authorization")
                    .ok_or("missing authorization")?;
                if got != auth {
                    return Err(format!("authorization {got} != {auth}"));
                }
            }
            if let Some(model) = &fixture.expect.request_model {
                let got = journal[0]
                    .body
                    .as_ref()
                    .and_then(|b| b.get("model"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if got != model {
                    return Err(format!("request model {got} != {model}"));
                }
            }
            if let Outcome::SystemOne(resp) = outcome {
                let resp = *resp;
                if let Some(key) = &fixture.expect.unknown_answer_key {
                    match resp.answer(key) {
                        Some(Answer::Unknown) => {}
                        other => {
                            return Err(format!("answer {key} should be Unknown, got {other:?}"));
                        }
                    }
                }
                if let Some(nouls) = &fixture.expect.noul {
                    for (k, v) in nouls {
                        let got = resp.noul(k);
                        if got != Some(*v) {
                            return Err(format!("noul {k}: {got:?} != {v}"));
                        }
                    }
                }
                if let Some(expected) = fixture.expect.attempts {
                    if resp.meta.attempts != expected {
                        return Err(format!(
                            "meta.attempts {} != {expected}",
                            resp.meta.attempts
                        ));
                    }
                }
            }
            Ok(())
        }
        ("error", Err(err)) => {
            if let Some(kind) = &fixture.expect.error_kind {
                match (kind.as_str(), &err) {
                    ("bad_request", Error::Api(api)) if api.kind == ApiErrorKind::BadRequest => {}
                    ("authentication", Error::Api(api))
                        if api.kind == ApiErrorKind::Authentication => {}
                    ("rate_limit", Error::Api(api)) if api.kind == ApiErrorKind::RateLimit => {}
                    ("unexpected_shape", Error::UnexpectedShape { .. }) => {}
                    other => return Err(format!("error_kind {kind} did not match {other:?}")),
                }
            }
            Ok(())
        }
        ("ok", Err(err)) => Err(format!("expected ok, got error {err}")),
        ("error", Ok(_)) => Err("expected error, got ok".into()),
        (other, _) => Err(format!("unknown expect.result {other}")),
    }
}
