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

#[cfg(feature = "benchmark")]
#[path = "../../transfer.rs"]
mod transfer;

/* === Re-Exports === */

pub use error::Error;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
#[cfg(feature = "benchmark")]
use harmos_sidecar as carrier;
use harmos_sidecar::Supervisor;
use harmos_sidecar::operations::{OperationPolicy, ServingError, StreamSettings, WireStreamLimits, WireTable};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

#[cfg(feature = "benchmark")]
type TransferContext = harmos_sidecar::Supervisor;

/* === Structs === */

pub struct Resources {
    analysis: tokio::sync::Semaphore,
}

pub struct Plugin {
    #[cfg(feature = "benchmark")]
    transfer: Arc<transfer::Store>,
    resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_sidecar::operations::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = schematic_domain::operations::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[cfg_attr(not(feature = "benchmark"), harmos_sidecar::plugin(id = "schematic-tools", provides = schematic_domain::operations::Native, requires = schematic_domain::operations::Requires, streams(schematic_domain::model::schematic::progress::AnalysisProgress)))]
#[cfg_attr(feature = "benchmark", harmos_sidecar::plugin(id = "schematic-tools", provides = schematic_domain::operations::transfer::Native, requires = schematic_domain::operations::transfer::NativeRequires, streams(schematic_domain::model::schematic::progress::AnalysisProgress)))]
impl harmos_sidecar::Plugin for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Supervisor;
    type Resources = Resources;

    fn prepare(_: ()) -> Result<Self, Error> {
        Ok(Self {
            #[cfg(feature = "benchmark")]
            transfer: Arc::new(transfer::Store::default()),
            resources: Arc::new(Resources { analysis: tokio::sync::Semaphore::new(1) }),
        })
    }

    fn resources(&self) -> Arc<Resources> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(&self, table: &mut WireTable<Supervisor>) -> Result<(), ServingError> {
        #[cfg(feature = "benchmark")]
        {
            let direction = StreamSettings { items: NonZeroUsize::new(4).unwrap(), bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap() };
            let limits = WireStreamLimits { input: direction, output: direction };
            table.bind_streaming_operations::<transfer::Operations, transfer::Store>(Arc::clone(&self.transfer), limits)?;
        }
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
