//! Rust client for [TypeSafe](https://typesafe.ai)'s System One API.
//!
//! System One evaluates a `state` against a map of named questions (`noul`,
//! `choice`, `score`) and returns one typed answer per question. This crate is
//! the HTTP client: configuration, retries, errors, `POST /v1/systemone`, and
//! `GET /v1/models`.
//!
//! This is a community SDK, not an official TypeSafe product. Env vars, defaults,
//! retries, identification headers, and error kinds match the official Python
//! and TypeScript SDKs.
//!
//! Set `TYPESAFE_API_KEY`. Optional: `TYPESAFE_BASE_URL`, `TYPESAFE_DEFAULT_MODEL`.
//!
//! # Quick start
//!
//! ```no_run
//! use typesafe_rs::{questions, Client, Question};
//!
//! # async fn run() -> Result<(), typesafe_rs::Error> {
//! let client = Client::from_env()?;
//! let response = client
//!     .system_one(
//!         "Help! My payouts have been failing for 3 days.",
//!         questions! {
//!             "urgent" => Question::noul("Does this convey urgency?"),
//!         },
//!     )
//!     .await?;
//! println!("{:?}", response.noul("urgent"));
//! # Ok(())
//! # }
//! ```
//!
//! For tests, use [`typesafe-rs-mock`](https://docs.rs/typesafe-rs-mock) instead
//! of the live API. See the `quickstart` example.
//!
//! # Crate features
//!
//! | Feature | Default | Enables |
//! |---|---|---|
//! | `rustls` | yes | TLS via rustls (platform verifier) |
//! | `native-tls` | no | Platform TLS instead of, or in addition to, rustls |
//! | `tracing` | yes | `typesafe.request` spans and `retry_scheduled` events |
//! | `blocking` | no | [`BlockingClient`] |
//!
//! Enable `rustls` (the default) or `native-tls`. HTTPS will not compile with
//! neither.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![warn(missing_debug_implementations)]

#[cfg(not(any(feature = "rustls", feature = "native-tls")))]
compile_error!("Enable the `rustls` feature (default) or `native-tls`.");

mod backend;
#[cfg(feature = "blocking")]
mod blocking;
mod client;
mod config;
mod error;
mod headers;
mod retry;
mod transport;
pub mod types;

pub use backend::Backend;
#[cfg(feature = "blocking")]
#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
pub use blocking::{BlockingClient, BlockingModels};
pub use client::{Client, Models};
pub use config::{
    CallOptions, ClientConfig, DEFAULT_BASE_URL, DEFAULT_MODEL, DEFAULT_TIMEOUT, ENV_API_KEY,
    ENV_BASE_URL, ENV_DEFAULT_MODEL, SecretString,
};
pub use error::{ApiError, ApiErrorKind, Error, ErrorBody, TransportError};
pub use http::{HeaderMap, HeaderValue, StatusCode};
pub use indexmap::IndexMap;
pub use retry::{RetryPolicy, StatusSet};
pub use types::{
    Answer, ChoiceView, Entry, ListModelsResponse, MAX_CHOICE_OPTIONS, ModelCard, NoulCriteria,
    NoulView, Question, Questions, ResponseMeta, ScoreView, SystemOneRequest, SystemOneResponse,
    Usage,
};
pub use url::Url;

/// Crate version, used in `User-Agent` and `X-TypeSafe-SDK`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Result alias for this crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Build an ordered map of named questions.
///
/// # Examples
///
/// ```
/// use typesafe_rs::{questions, Question};
///
/// let qs = questions! {
///     "urgent" => Question::noul("Does this convey urgency?"),
///     "team" => Question::choice("Which team?")
///         .option("billing", "Payments")
///         .option("technical", "Bugs"),
/// };
/// assert_eq!(qs.len(), 2);
/// ```
#[macro_export]
macro_rules! questions {
    ( $($key:expr => $value:expr),* $(,)? ) => {{
        let mut map = $crate::IndexMap::new();
        $(
            map.insert(
                ::std::string::ToString::to_string(&$key),
                ::core::convert::Into::<$crate::Question>::into($value),
            );
        )*
        map
    }};
}
