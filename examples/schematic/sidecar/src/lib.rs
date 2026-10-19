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
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use harmos_sidecar::Supervisor;
use harmos_sidecar::operations::{OperationPolicy, ServingError, StreamSettings, WireStreamLimits, WireTable};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Resources {
    analysis: tokio::sync::Semaphore,
}

pub struct Plugin {
    resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_sidecar::operations::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = schematic_domain::operations::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_sidecar::plugin(id = "schematic-tools", provides = schematic_domain::operations::Native, requires = schematic_domain::operations::Requires, streams(schematic_domain::model::schematic::progress::AnalysisProgress))]
impl harmos_sidecar::Plugin for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Supervisor;
    type Resources = Resources;

    fn prepare(_: ()) -> Result<Self, Error> {
        Ok(Self { resources: Arc::new(Resources { analysis: tokio::sync::Semaphore::new(1) }) })
    }

    fn resources(&self) -> Arc<Resources> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(&self, table: &mut WireTable<Supervisor>) -> Result<(), ServingError> {
        let direction = StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(4096).unwrap() };
        let limits = WireStreamLimits { input: direction, output: direction };
        table.bind_streaming_operations::<operations::Operations, Resources>(self.resources(), limits)
    }

    async fn initialize(&mut self, _: &Supervisor) -> Result<(), Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Supervisor) -> Result<(), Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
