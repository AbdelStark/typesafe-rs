use serde::Serialize;

use crate::Client;
use crate::config::{CallOptions, ClientConfig};
use crate::error::Error;
use crate::types::{ModelCard, Questions, SystemOneRequest, SystemOneResponse};

/// Blocking wrapper around [`Client`] using a current-thread Tokio runtime.
///
/// Do not use this type from inside an existing Tokio runtime (`block_on` will panic).
#[derive(Debug)]
pub struct BlockingClient {
    client: Client,
    rt: tokio::runtime::Runtime,
}

impl BlockingClient {
    /// Build a blocking client. See [`Client::new`].
    pub fn new(config: ClientConfig) -> Result<Self, Error> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| Error::InvalidRequest(format!("failed to create runtime: {err}")))?;
        let client = Client::new(config)?;
        Ok(Self { client, rt })
    }

    /// [`Self::new`] using process environment fallbacks.
    pub fn from_env() -> Result<Self, Error> {
        Self::new(ClientConfig::default())
    }

    /// Like [`Client::new_with_env`].
    pub fn new_with_env(
        config: ClientConfig,
        lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, Error> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| Error::InvalidRequest(format!("failed to create runtime: {err}")))?;
        let client = Client::new_with_env(config, lookup)?;
        Ok(Self { client, rt })
    }

    /// Blocking [`Client::system_one`].
    pub fn system_one(
        &self,
        state: impl Serialize,
        questions: Questions,
    ) -> Result<SystemOneResponse, Error> {
        self.rt.block_on(self.client.system_one(state, questions))
    }

    /// Blocking [`Client::system_one_with`].
    pub fn system_one_with(
        &self,
        req: &SystemOneRequest,
        opts: CallOptions,
    ) -> Result<SystemOneResponse, Error> {
        self.rt.block_on(self.client.system_one_with(req, opts))
    }

    /// Access the Models resource.
    #[must_use]
    pub fn models(&self) -> BlockingModels<'_> {
        BlockingModels { client: self }
    }

    /// Blocking [`Client::warm_up`].
    pub fn warm_up(&self) -> Result<(), Error> {
        self.rt.block_on(self.client.warm_up())
    }

    /// Default model used when a request omits `model`.
    #[must_use]
    pub fn default_model(&self) -> &str {
        self.client.default_model()
    }
}

/// Blocking Models API resource.
#[derive(Debug)]
pub struct BlockingModels<'a> {
    client: &'a BlockingClient,
}

impl BlockingModels<'_> {
    /// List models available to the account.
    pub fn list(&self) -> Result<Vec<ModelCard>, Error> {
        self.client.rt.block_on(self.client.client.models().list())
    }
}
