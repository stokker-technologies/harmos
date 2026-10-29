/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(feature = "benchmark")]
mod benchmark;
#[cfg(not(feature = "benchmark"))]
mod registration;

/* === Re-Exports === */

pub use std::io::Error;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
#[cfg(feature = "benchmark")]
use benchmark as registration;
use harmos::operations::{OperationPolicy, StreamSettings, WireStreamLimits};
use harmos::sidecar::Supervisor;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Plugin {
    analysis: Arc<schematic_operations::resources::AnalysisResources>,
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
    type Context = Supervisor;

    async fn prepare(_: (), setup: &mut harmos::Setup<Self>) -> Result<Self, Error> {
        let analysis = Arc::new(schematic_operations::resources::AnalysisResources::default());
        let plugin = Self { analysis };
        setup.resource::<schematic_operations::resources::AnalysisResources>(Arc::clone(&plugin.analysis));
        registration::prepare(setup);
        let direction = StreamSettings {
            items: NonZeroUsize::new(4).unwrap(),
            bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
        };
        setup.settings().stream_limits(WireStreamLimits { input: direction, output: direction }).map_err(Error::other)?;
        Ok(plugin)
    }
}

#[harmos::sidecar(id = "schematic_tools", requires = (), streams(schematic_domain::model::schematic::progress::AnalysisProgress))]
impl harmos::Plugin for Plugin {
    const CATALOGS: &'static [harmos::Catalog] = &[registration::CATALOG];
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
