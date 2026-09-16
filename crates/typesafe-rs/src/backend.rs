use std::future::Future;

use crate::Client;
use crate::config::CallOptions;
use crate::error::Error;
use crate::types::{SystemOneRequest, SystemOneResponse};

/// Pluggable System One evaluator.
///
/// [`Client`](crate::Client) implements this so callers can depend on the trait
/// rather than the HTTP type. Additional backends (LLM, Cascade) are planned.
pub trait Backend: Send + Sync {
    /// Stable backend name, e.g. `"typesafe"`.
    fn name(&self) -> &str;

    /// Evaluate `req` with per-call options.
    fn system_one(
        &self,
        req: &SystemOneRequest,
        opts: &CallOptions,
    ) -> impl Future<Output = Result<SystemOneResponse, Error>> + Send;
}

impl Backend for Client {
    fn name(&self) -> &str {
        "typesafe"
    }

    fn system_one(
        &self,
        req: &SystemOneRequest,
        opts: &CallOptions,
    ) -> impl Future<Output = Result<SystemOneResponse, Error>> + Send {
        let this = self.clone();
        let req = req.clone();
        let opts = opts.clone();
        async move { this.system_one_with(&req, opts).await }
    }
}
