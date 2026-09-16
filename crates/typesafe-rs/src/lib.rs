//! Community Rust SDK for TypeSafe's System One API.
//!
//! This crate is **not** an official TypeSafe product. Behaviour marked
//! **\[parity\]** in `SPEC.md` matches the official Python and TypeScript SDKs.
//!
//! # Quick start
//!
//! ```no_run
//! use typesafe_rs::{questions, Client, ClientConfig, Question};
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
//! For local tests, run against the `typesafe-rs-mock` crate instead of the live
//! API. See the `quickstart` example.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

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
