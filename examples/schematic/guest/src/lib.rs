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

pub use std::io::Error;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
#[cfg(feature = "benchmark")]
use std::sync::Arc;

// Third Party Imports

// Local Imports
use harmos::guest::kit::Context;
use harmos::operations::{OperationPolicy, StreamSettings, WireStreamLimits};
#[cfg(not(feature = "benchmark"))]
use schematic_operations::Operations;
#[cfg(feature = "benchmark")]
use schematic_operations::transfer::Guest as Operations;

/* === Constants === */

/* === Macros === */

harmos::guest::export_plugin!(Plugin, capacity = 8);

/* === Type Aliases === */

/* === Structs === */

pub struct Plugin {
    #[cfg(feature = "benchmark")]
    transfer: Arc<schematic_operations::transfer::Store>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos::builtins::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = schematic_operations::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos::Lifecycle for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Context;

    async fn prepare(_: (), setup: &mut harmos::Setup<Self>) -> Result<Self, Error> {
        let plugin = Self {
            #[cfg(feature = "benchmark")]
            transfer: Arc::new(schematic_operations::transfer::Store::default()),
        };
        #[cfg(feature = "benchmark")]
        setup.resource::<schematic_operations::transfer::Store>(Arc::clone(&plugin.transfer));
        let direction = StreamSettings {
            items: NonZeroUsize::new(4).unwrap(),
            bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
        };
        setup.settings().stream_limits(WireStreamLimits { input: direction, output: direction }).map_err(Error::other)?;
        Ok(plugin)
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

#[harmos::guest(id = "schematic_tools", requires = ())]
impl harmos::Plugin for Plugin {
    const CATALOGS: &'static [harmos::Catalog] = &[Operations::CATALOG];
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
