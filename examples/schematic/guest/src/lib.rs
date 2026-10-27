/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

pub use std::convert::Infallible as Error;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use harmos_guest::kit::Context;
use harmos_guest::operations::{OperationPolicy, ServingError, StreamSettings, WireStreamLimits, WireTable};

/* === Constants === */

/* === Macros === */

harmos_guest::export_plugin!(Plugin, capacity = 8);

/* === Type Aliases === */

/* === Structs === */

pub struct Plugin {
    #[cfg(feature = "benchmark")]
    transfer: Arc<schematic_operations::transfer::Store>,
    resources: Arc<()>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_guest::builtins::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = schematic_operations::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[cfg_attr(not(feature = "benchmark"), harmos_guest::plugin(id = "schematic_tools", provides = schematic_operations::Operations, requires = ()))]
#[cfg_attr(feature = "benchmark", harmos_guest::plugin(id = "schematic_tools", provides = schematic_operations::transfer::Guest, requires = ()))]
impl harmos_guest::Plugin for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Context;
    type Resources = ();

    fn prepare(_: ()) -> Result<Self, Error> {
        Ok(Self {
            #[cfg(feature = "benchmark")]
            transfer: Arc::new(schematic_operations::transfer::Store::default()),
            resources: Arc::new(()),
        })
    }

    fn resources(&self) -> Arc<()> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(&self, table: &mut WireTable) -> Result<(), ServingError> {
        let dependencies = harmos_guest::Dependencies::new();
        #[cfg(feature = "benchmark")]
        dependencies.insert(self.transfer.clone()).expect("each dependency is inserted once");
        let direction = StreamSettings {
            items: NonZeroUsize::new(4).unwrap(),
            bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
        };
        table.stream_limits(WireStreamLimits { input: direction, output: direction })?;
        #[cfg(feature = "benchmark")]
        let batch = schematic_operations::transfer::Guest::registration(dependencies);
        #[cfg(not(feature = "benchmark"))]
        let batch = schematic_operations::Operations::registration(dependencies);
        table.register(batch)
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
