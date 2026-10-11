/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod error;
pub mod operations;
pub mod streams;

/* === Re-Exports === */
pub use error::{Error, Result};

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::operations::{OperationPolicy, RetryPolicy, ServingError, WireTable};
use harmos::sidecar::{Refusal, Supervisor};
use minibench_contract::probe::{Operations, Settings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Resources {
    pub device: String,
}

pub struct ProbeBridge {
    resources: std::sync::Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for ProbeBridge {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: std::time::Duration::from_secs(30),
        attempt_timeout: std::time::Duration::from_secs(5),
        retries: RetryPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(
    description = "Reads a native probe and publishes its measurements.",
    id = "probe-bridge",
    provides = Operations,
    requires(harmos::operations::host::Publish),
    streams(streams::Reading),
    restart(
        credit = 3,
        sustained_health_ms = 30_000,
        backoff_base_ms = 250,
        backoff_cap_ms = 10_000
    )
)]
impl harmos::Plugin for ProbeBridge {
    type Config = Settings;
    type Error = Refusal;
    type Context = Supervisor;
    type Resources = Resources;
    fn prepare(config: Settings) -> std::result::Result<Self, Refusal> {
        Ok(Self {
            resources: std::sync::Arc::new(Resources {
                device: config.device,
            }),
        })
    }
    fn resources(&self) -> std::sync::Arc<Resources> {
        std::sync::Arc::clone(&self.resources)
    }
    async fn initialize(&mut self, _: &Supervisor) -> std::result::Result<(), Refusal> {
        Ok(())
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> std::result::Result<(), ServingError> {
        table.bind_operation::<operations::sample::Sample>(self.resources())
    }
    async fn shutdown(&mut self, _: &Supervisor) -> std::result::Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> std::result::Result<(), Refusal> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
