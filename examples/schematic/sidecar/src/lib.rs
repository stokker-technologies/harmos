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
use harmos_sidecar::Supervisor;
use harmos_sidecar::operations::{OperationPolicy, ServingError, StreamSettings, WireStreamLimits, WireTable};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Resources {
    analysis: Arc<schematic_operations::resources::AnalysisResources>,
}

pub struct Plugin {
    hash: Arc<schematic_operations::resources::HashProvider>,
    #[cfg(feature = "benchmark")]
    transfer: Arc<schematic_operations::transfer::Store>,
    resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_sidecar::builtins::lifecycle::Policy for Plugin {
    const INITIALIZATION_POLICY: OperationPolicy = schematic_operations::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Self::INITIALIZATION_POLICY;
}

#[cfg_attr(not(feature = "benchmark"), harmos_sidecar::plugin(id = "schematic_tools", provides = schematic_operations::Native, requires = schematic_operations::Requires, streams(schematic_domain::model::schematic::progress::AnalysisProgress)))]
#[cfg_attr(feature = "benchmark", harmos_sidecar::plugin(id = "schematic_tools", provides = schematic_operations::transfer::Native, requires = schematic_operations::transfer::NativeRequires, streams(schematic_domain::model::schematic::progress::AnalysisProgress)))]
impl harmos_sidecar::Plugin for Plugin {
    type Config = ();
    type Error = Error;
    type Context = Supervisor;
    type Resources = Resources;

    fn prepare(_: ()) -> Result<Self, Error> {
        Ok(Self {
            hash: Arc::new(schematic_operations::resources::HashProvider {
                hasher: Arc::new(|context, request| {
                    Box::pin(async move {
                        use harmos_sidecar::Host;
                        let host = context.dependency::<harmos_sidecar::Supervisor>()?;
                        let response = host
                            .call::<harmos_sidecar::builtins::storage::HashFile>(request)
                            .await
                            .map_err(|error| schematic_operations::Error::Host(Box::new(error)))?;
                        Ok(*response)
                    })
                }),
            }),
            #[cfg(feature = "benchmark")]
            transfer: Arc::new(schematic_operations::transfer::Store::default()),
            resources: Arc::new(Resources {
                analysis: Arc::new(schematic_operations::resources::AnalysisResources::new(Arc::new(|context, progress| {
                    Box::pin(async move {
                        use harmos_sidecar::Host;
                        let host = context.dependency::<harmos_sidecar::Supervisor>()?;
                        host.publish_on::<schematic_domain::model::schematic::progress::AnalysisProgress>(&progress)
                            .await
                            .map_err(|error| schematic_operations::Error::Host(Box::new(error)))
                    })
                }))),
            }),
        })
    }

    fn resources(&self) -> Arc<Resources> {
        Arc::clone(&self.resources)
    }

    fn bind_operations(&self, table: &mut WireTable) -> Result<(), ServingError> {
        let mut dependencies = harmos_sidecar::Dependencies::new();
        dependencies.insert(self.hash.clone()).expect("each dependency is inserted once");
        dependencies.insert(self.resources.analysis.clone()).expect("each dependency is inserted once");
        #[cfg(feature = "benchmark")]
        dependencies.insert(self.transfer.clone()).expect("each dependency is inserted once");
        let direction = StreamSettings { items: NonZeroUsize::new(4).unwrap(), bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap() };
        table.stream_limits(WireStreamLimits { input: direction, output: direction })?;
        #[cfg(feature = "benchmark")]
        let batch = schematic_operations::transfer::Native::registration(dependencies);
        #[cfg(not(feature = "benchmark"))]
        let batch = schematic_operations::Native::registration(dependencies);
        table.register(batch)
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
