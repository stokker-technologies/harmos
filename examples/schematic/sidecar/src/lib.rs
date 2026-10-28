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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use harmos::operations::{OperationPolicy, StreamSettings, WireStreamLimits};
use harmos::sidecar::Supervisor;
#[cfg(not(feature = "benchmark"))]
use schematic_operations::Native as Operations;
#[cfg(feature = "benchmark")]
use schematic_operations::transfer::Native as Operations;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Plugin {
    #[cfg(feature = "benchmark")]
    transfer: Arc<schematic_operations::transfer::Store>,

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
        #[cfg(feature = "benchmark")]
        let transfer = Arc::new(schematic_operations::transfer::Store::default());

        let plugin = Self {
            #[cfg(feature = "benchmark")]
            transfer,
            analysis,
        };
        setup.resource::<schematic_operations::resources::AnalysisResources>(Arc::clone(&plugin.analysis));
        #[cfg(feature = "benchmark")]
        setup.resource::<schematic_operations::transfer::Store>(Arc::clone(&plugin.transfer));
        let direction = StreamSettings {
            items: NonZeroUsize::new(4).unwrap(),
            bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
        };
        setup.settings().stream_limits(WireStreamLimits { input: direction, output: direction }).map_err(Error::other)?;
        Ok(plugin)
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

#[harmos::sidecar(id = "schematic_tools", requires = (), streams(schematic_domain::model::schematic::progress::AnalysisProgress))]
impl harmos::Plugin for Plugin {
    const CATALOGS: &'static [harmos::Catalog] = &[Operations::CATALOG];
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
