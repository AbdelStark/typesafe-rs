use std::future::Future;

use crate::Client;
use crate::config::CallOptions;
use crate::error::Error;
use crate::types::{SystemOneRequest, SystemOneResponse};

/// Pluggable System One evaluator.
///
/// A thin v0.1 surface so downstream crates (for example `s1-rs`) can depend on
/// a trait rather than a concrete HTTP client. LLM and Cascade backends land in v0.3.
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
