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
mod operations;

/* === Re-Exports === */
pub use error::Error;

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// Third Party Imports

// Local Imports
use crate::contract::{Operations, POLICY};
use harmos::operations::{OperationPolicy, ServingError, WireTable};
use harmos::sidecar::Supervisor;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */
#[derive(Default)]
pub struct Resources {
    constructed: AtomicU64,
    completed: AtomicU64,
    finalized: AtomicU64,
}

pub struct Plugin {
    resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::operations::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = POLICY;
}

#[harmos::plugin(id = "lifecycle-worker", provides = Operations, requires = ())]
impl harmos::Plugin for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Supervisor;
    type Resources = Resources;

    fn prepare(_: ()) -> Result<Self> {
        Ok(Self {
            resources: Arc::new(Resources::default()),
        })
    }
    fn resources(&self) -> Arc<Resources> {
        Arc::clone(&self.resources)
    }
    fn bind_operations(
        &self,
        table: &mut WireTable<Supervisor>,
    ) -> std::result::Result<(), ServingError> {
        table.bind_operation::<operations::exchange::Exchanging>(self.resources())
    }
    async fn initialize(&mut self, _: &Supervisor) -> Result<()> {
        Ok(())
    }
    async fn stop(&mut self, _: &Supervisor) -> Result<()> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<()> {
        let constructed = self.resources.constructed.load(Ordering::SeqCst);
        let completed = self.resources.completed.load(Ordering::SeqCst);
        let finalized = self.resources.finalized.load(Ordering::SeqCst);
        if constructed != finalized {
            return Err(Error::Unsettled {
                constructed,
                finalized,
            });
        }
        tracing::debug!(
            constructed,
            completed,
            finalized,
            "worker operations settled"
        );
        eprintln!(
            "worker-cleaned constructed={constructed} completed={completed} finalized={finalized}"
        );
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
