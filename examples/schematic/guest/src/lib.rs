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
use harmos_guest::Context;
use harmos_guest::operations::{OperationPolicy, ServingError, StreamSettings, WireStreamLimits, WireTable};

/* === Constants === */

/* === Macros === */

harmos_guest::export_plugin!(Plugin, capacity = 8);

/* === Type Aliases === */

/* === Structs === */

pub struct Plugin {
    resources: Arc<()>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_guest::operations::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = schematic_domain::operations::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[harmos_guest::plugin(id = "schematic-tools", provides = schematic_domain::operations::Operations, requires = ())]
impl harmos_guest::Plugin for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Context;
    type Resources = ();

    fn prepare(_: ()) -> Result<Self, Error> {
        Ok(Self { resources: Arc::new(()) })
    }

    fn resources(&self) -> Arc<()> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(&self, table: &mut WireTable<Context>) -> Result<(), ServingError> {
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
