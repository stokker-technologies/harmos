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
use harmos_guest as carrier;
use harmos_guest::Context;
use harmos_guest::operations::{OperationPolicy, ServingError, StreamSettings, WireStreamLimits, WireTable};

/* === Constants === */

/* === Macros === */

harmos_guest::export_plugin!(Plugin, capacity = 8);

/* === Type Aliases === */

#[cfg(feature = "benchmark")]
type TransferContext = harmos_guest::Context;

/* === Structs === */

pub struct Plugin {
    #[cfg(feature = "benchmark")]
    transfer: Arc<transfer::Store>,
    resources: Arc<()>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_guest::operations::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = schematic_domain::operations::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[cfg_attr(not(feature = "benchmark"), harmos_guest::plugin(id = "schematic-tools", provides = schematic_domain::operations::Operations, requires = ()))]
#[cfg_attr(feature = "benchmark", harmos_guest::plugin(id = "schematic-tools", provides = schematic_domain::operations::transfer::Guest, requires = schematic_domain::operations::transfer::GuestRequires))]
impl harmos_guest::Plugin for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Context;
    type Resources = ();

    fn prepare(_: ()) -> Result<Self, Error> {
        Ok(Self {
            #[cfg(feature = "benchmark")]
            transfer: Arc::new(transfer::Store::default()),
            resources: Arc::new(()),
        })
    }

    fn resources(&self) -> Arc<()> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(&self, table: &mut WireTable<Context>) -> Result<(), ServingError> {
        #[cfg(feature = "benchmark")]
        {
            let direction = StreamSettings { items: NonZeroUsize::new(4).unwrap(), bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap() };
            let limits = WireStreamLimits { input: direction, output: direction };
            table.bind_streaming_operations::<transfer::Operations, transfer::Store>(Arc::clone(&self.transfer), limits)?;
        }
        let direction = StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(4096).unwrap() };
        let limits = WireStreamLimits { input: direction, output: direction };
        table.bind_streaming_operations::<operations::Operations, ()>(self.resources(), limits)
    }

    async fn initialize(&mut self, _: &Context) -> Result<(), Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Context) -> Result<(), Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
