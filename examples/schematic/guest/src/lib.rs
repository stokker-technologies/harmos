/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub use std::io::Error;

use std::num::NonZeroUsize;
#[cfg(feature = "benchmark")]
use std::sync::Arc;

use harmos::guest::kit::Context;
use harmos::operations::{OperationPolicy, StreamSettings, WireStreamLimits};
#[cfg(not(feature = "benchmark"))]
use schematic_operations::Operations;
#[cfg(feature = "benchmark")]
use schematic_operations::transfer::Guest as Operations;

harmos::guest::export_plugin!(Plugin, capacity = 8);

pub struct Plugin {
    #[cfg(feature = "benchmark")]
    transfer: Arc<schematic_operations::transfer::Store>,
}

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
}

#[harmos::guest(id = "schematic_tools", requires = ())]
impl harmos::Plugin for Plugin {
    const CATALOGS: &'static [harmos::Catalog] = &[Operations::CATALOG];
}
