/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod config;
mod error;
mod operations;
mod streams;
mod transactions;

/* === Re-Exports === */
pub use error::{Error, Result};

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use config::AutorouteConfig;
use harmos::guest::Context;
use harmos::operations::{OperationPolicy, RetryPolicy, ServingError, WireTable, host};
use minischematic_contract::autoroute::{NetName, Operations, RenameNet};

/* === Constants === */

/* === Macros === */
harmos::guest::export_plugin!(Autoroute, capacity = 64);

/* === Type Aliases === */

/* === Structs === */
struct Autoroute {
    resources: std::sync::Arc<()>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Autoroute {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: std::time::Duration::from_secs(30),
        attempt_timeout: std::time::Duration::from_secs(30),
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(
    id = "autoroute",
    provides = Operations,
    requires(host::Publish, host::Invoke),
    streams(streams::progress::Progress)
)]
impl harmos::Plugin for Autoroute {
    type Config = AutorouteConfig;
    type Error = Error;
    type Context = Context;
    type Resources = ();
    fn prepare(_: AutorouteConfig) -> Result<Self> {
        Ok(Self {
            resources: std::sync::Arc::new(()),
        })
    }
    fn resources(&self) -> std::sync::Arc<Self::Resources> {
        std::sync::Arc::clone(&self.resources)
    }
    async fn initialize(&mut self, context: &Context) -> Result<()> {
        initialize(context).await
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Context>,
    ) -> std::result::Result<(), ServingError> {
        table.bind_operation::<operations::echo::EchoOperation>(self.resources())
    }
    async fn shutdown(&mut self, _: &Context) -> Result<()> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
#[tracing::instrument(level = "debug", skip_all)]
async fn initialize(core: &Context) -> Result<()> {
    streams::progress::publish(core, 10).await?;

    let change = RenameNet {
        net: 1,
        to: NetName::plain("VCC"),
    };
    let first = transactions::rename(core, &change).await?;
    let retry = transactions::rename(core, &change).await?;
    if first.position != retry.position || first.correlation != retry.correlation {
        return Err(Error::RetryMismatch);
    }

    streams::progress::publish(core, 100).await
}

/* === Tests === */
