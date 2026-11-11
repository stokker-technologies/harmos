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
use harmos_guest::kit::Context;
use harmos_guest::operations::{OperationPolicy, StreamSettings, WireStreamLimits};
#[cfg(not(feature = "benchmark"))]
use schematic_operations::guest::OPERATIONS_CATALOG;
#[cfg(feature = "benchmark")]
use schematic_operations::guest::transfer::GUEST_CATALOG as OPERATIONS_CATALOG;

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_guest::operations::RetryPolicy::Never,
};

/* === Macros === */

harmos_guest::export_plugin!(Plugin, capacity = 8);

/* === Type Aliases === */

/* === Structs === */

pub struct Plugin {
    #[cfg(feature = "benchmark")]
    transfer: Arc<schematic_operations::guest::transfer::Store>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_guest::builtins::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

impl harmos_guest::Lifecycle for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Context;

    async fn prepare(_: (), setup: &mut harmos_guest::Setup<Self>) -> Result<Self, Error> {
        let plugin = Self {
            #[cfg(feature = "benchmark")]
            transfer: Arc::new(schematic_operations::guest::transfer::Store::default()),
        };
        #[cfg(feature = "benchmark")]
        setup.resource::<schematic_operations::guest::transfer::Store>(Arc::clone(&plugin.transfer));
        let direction = StreamSettings {
            items: NonZeroUsize::new(4).unwrap(),
            bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
        };
        setup.settings().stream_limits(WireStreamLimits { input: direction, output: direction }).map_err(Error::other)?;
        Ok(plugin)
    }
}

#[harmos_guest::guest(id = "schematic_tools", requires = ())]
impl harmos_guest::Plugin for Plugin {
    const CATALOGS: &'static [harmos_guest::CatalogDescription] = &[OPERATIONS_CATALOG];
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
