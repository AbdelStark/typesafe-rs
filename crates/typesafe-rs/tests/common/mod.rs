#![allow(dead_code)]

use std::time::Duration;

use typesafe_rs::{Client, ClientConfig, RetryPolicy};
use typesafe_rs_mock::MockServer;

pub fn test_config(mock: &MockServer) -> ClientConfig {
    ClientConfig {
        api_key: Some("test".into()),
        base_url: Some(mock.url()),
        default_model: Some("jev-latest".into()),
        retry: RetryPolicy {
            backoff_initial: Duration::from_millis(5),
            backoff_max: Duration::from_millis(20),
            backoff_jitter: 0.0,
            ..RetryPolicy::default()
        },
        ..ClientConfig::default()
    }
}

pub fn test_client(mock: &MockServer) -> Client {
    Client::new(test_config(mock)).expect("test client")
}

pub fn default_retry_client(mock: &MockServer) -> Client {
    Client::new(ClientConfig {
        api_key: Some("test".into()),
        base_url: Some(mock.url()),
        default_model: Some("jev-latest".into()),
        ..ClientConfig::default()
    })
    .expect("client")
}
