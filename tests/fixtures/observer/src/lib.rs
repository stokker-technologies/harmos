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
pub mod listeners;
pub mod objects;
pub mod observers;
pub mod operations;

/* === Re-Exports === */
pub use error::{Error, Result};

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::operations::{OperationPolicy, RestartPolicy, RetryPolicy, ServingError, WireTable};
use harmos::plugin::contract::{Attachment, Declared};
use harmos::plugin::sidecar::{ObserverAttachment, Refusal, Supervisor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Settings {
    pub instrument: String,
}

pub struct Resources {
    pub instrument: String,
}

pub struct StandaloneBridge {
    resources: std::sync::Arc<Resources>,
    observers: Option<
        std::sync::Arc<ObserverAttachment<Resources, harmos::plugin::sidecar::Publisher<Self>>>,
    >,
}

/* === Enums === */
/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for StandaloneBridge {
    const INITIALIZATION_POLICY: OperationPolicy = OperationPolicy {
        timeout: std::time::Duration::from_secs(30),
        attempt_timeout: std::time::Duration::from_secs(5),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos::plugin(
    description = "Reads an instrument with declared observer and listener callbacks.",
    id = "standalone-bridge",
    streams(crate::operations::read::Sample, observers::DriftRow),
    provides = operations::Operations,
    requires(harmos::operations::host::Publish),
    observers = observers::Observers,
    listeners = listeners::Listeners,
    observer_control = observers::controls::Observe,
    restart(
        credit = 3,
        sustained_health_ms = 30_000,
        backoff_base_ms = 250,
        backoff_cap_ms = 10_000
    )
)]
impl harmos::Plugin for StandaloneBridge {
    type Config = Settings;
    type Error = Refusal;
    type Context = Supervisor;
    type Resources = Resources;

    fn prepare(config: Settings) -> std::result::Result<Self, Refusal> {
        if config.instrument.is_empty() {
            return Err(Refusal::of("a bridge is wired to a named instrument"));
        }
        Ok(Self {
            resources: std::sync::Arc::new(Resources {
                instrument: config.instrument,
            }),
            observers: None,
        })
    }

    async fn initialize(&mut self, _: &Supervisor) -> std::result::Result<(), Refusal> {
        self.observers = Some(ObserverAttachment::from_catalog::<observers::Observers>(
            self.resources(),
            <Self as Declared>::STREAMS,
        )?);
        Ok(())
    }

    fn resources(&self) -> std::sync::Arc<Resources> {
        std::sync::Arc::clone(&self.resources)
    }

    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> std::result::Result<(), ServingError> {
        table.bind_operations::<operations::Implemented, _>(self.resources())?;
        self.observers
            .as_ref()
            .ok_or(ServingError::Catalog)?
            .bind::<observers::controls::Observe>(table)
    }

    fn attachment(&self) -> Option<std::sync::Arc<dyn Attachment<Supervisor>>> {
        self.observers
            .as_ref()
            .map(|observers| observers.clone() as std::sync::Arc<dyn Attachment<Supervisor>>)
    }

    async fn stop(&mut self, _: &Supervisor) -> std::result::Result<(), Refusal> {
        Ok(())
    }
    async fn cleanup(&mut self) -> std::result::Result<(), Refusal> {
        self.observers.take();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
