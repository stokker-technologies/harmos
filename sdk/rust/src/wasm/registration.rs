/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// WebAssembly Component Adapter

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use crate::{Context, Error, Plugin, Result, operation::LocalFuture};

use std::sync::Arc;

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_protocol::model::OperationPolicy;
use harmos_protocol::plugin::GuestRegistration;
use harmos_protocol::plugin::GuestRequirement;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Component-only lifecycle around the same plugin builder used by native authors.
pub struct Registration {
    pub(crate) plugin: Plugin,
    pub(crate) capacity: usize,
    pub(crate) initialize: Option<Arc<dyn Initialize>>,
    pub(crate) shutdown: Option<Arc<dyn Shutdown>>,
    pub(crate) cleanup: Vec<Box<dyn Fn() -> Result<()> + Send + Sync>>,
    initialization_policy: Option<OperationPolicy>,
    shutdown_policy: Option<OperationPolicy>,
}
struct InitializeHandler<H>(H);
struct ShutdownHandler<H>(H);

/* === Enums === */

/* === Traits === */

pub(crate) trait Initialize: Send + Sync {
    fn run<'a>(&'a self, context: &'a Context, configuration: Vec<u8>) -> LocalFuture<'a, Result<()>>;
}
pub(crate) trait Shutdown: Send + Sync {
    fn run<'a>(&'a self, context: &'a Context) -> LocalFuture<'a, Result<()>>;
}

/* === Implementations === */

impl Registration {
    pub fn new(plugin: Plugin) -> Self {
        Self {
            plugin,
            capacity: 1,
            initialize: None,
            shutdown: None,
            cleanup: Vec::new(),
            initialization_policy: None,
            shutdown_policy: None,
        }
    }
    pub fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }
    pub fn lifecycle_policy(mut self, initialization: OperationPolicy, shutdown: OperationPolicy) -> Self {
        self.initialization_policy = Some(initialization);
        self.shutdown_policy = Some(shutdown);
        self
    }
    pub fn on_initialize<H>(mut self, initialize: H) -> Self
    where
        H: for<'a> AsyncFn(&'a Context, Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        self.initialize = Some(Arc::new(InitializeHandler(initialize)));
        self
    }
    pub fn on_shutdown<H>(mut self, shutdown: H) -> Self
    where
        H: for<'a> AsyncFn(&'a Context) -> Result<()> + Send + Sync + 'static,
    {
        self.shutdown = Some(Arc::new(ShutdownHandler(shutdown)));
        self
    }
    /// Runs after shutdown settles, including a cancelled or failed shutdown.
    pub fn on_cleanup(mut self, cleanup: impl Fn() -> Result<()> + Send + Sync + 'static) -> Self {
        self.cleanup.push(Box::new(cleanup));
        self
    }
    /// Inspect metadata without entering any lifecycle or operation callback.
    pub fn metadata(&self) -> Result<GuestRegistration> {
        if self.capacity == 0 {
            return Err(Error::Invalid("Guest Operation Capacity Must Be Positive".into()));
        }
        if !self.plugin.observers.is_empty() || self.plugin.streams.iter().any(|stream| stream.producer.is_some()) {
            return Err(Error::Invalid("Guest Streams Publish Through Their Invocation Context".into()));
        }
        let declarations = self.plugin.declarations()?;
        let requirements = self
            .plugin
            .requirements
            .iter()
            .zip(&declarations.requires)
            .map(|(required, contract)| GuestRequirement {
                id: required.id.clone(),
                request: required.request.clone(),
                response: required.response.clone(),
                fingerprint: contract.fingerprint.clone(),
            })
            .collect();
        Ok(GuestRegistration {
            id: self.plugin.id.clone(),
            version: self.plugin.version.clone(),
            declarations: Some(declarations),
            initialization: self.initialization_policy,
            shutdown: self.shutdown_policy,
            requirements,
            message_bytes: harmos_protocol::transfer::message_bytes(&self.plugin.transfer)
                .filter(|bytes| bytes.get() <= u32::MAX as usize)
                .ok_or_else(|| {
                    Error::Invalid(
                        "Register Guest: MessageSettings.message_bytes cannot carry StorageTransferSettings.chunk_bytes plus framing"
                            .into(),
                    )
                })?
                .get() as u64,
        })
    }
}
impl<H> Initialize for InitializeHandler<H>
where
    H: for<'a> AsyncFn(&'a Context, Vec<u8>) -> Result<()> + Send + Sync + 'static,
{
    fn run<'a>(&'a self, context: &'a Context, configuration: Vec<u8>) -> LocalFuture<'a, Result<()>> {
        Box::pin((self.0)(context, configuration))
    }
}
impl<H> Shutdown for ShutdownHandler<H>
where
    H: for<'a> AsyncFn(&'a Context) -> Result<()> + Send + Sync + 'static,
{
    fn run<'a>(&'a self, context: &'a Context) -> LocalFuture<'a, Result<()>> {
        Box::pin((self.0)(context))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
