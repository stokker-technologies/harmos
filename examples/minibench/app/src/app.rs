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

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use crate::config::Config;
use crate::error::Error;
use crate::operations::report::Report;
use crate::resources::Resources;
use crate::services::probe;
use crate::streams::Streams;
use harmos::{Builder, Never};

/* === Constants === */
const PREPARATION_BYTES: NonZeroUsize = NonZeroUsize::new(4096).unwrap();

/* === Macros === */

/* === Type Aliases === */
pub type Origin = harmos::Origin<()>;

pub type Runtime = harmos::Runtime<Bench>;

pub type StateManager = harmos::StateManager<Bench>;

pub type StreamManager = harmos::StreamManager<Bench>;

#[cfg(feature = "sim")]
pub type Simulated = harmos::SimulatedRuntime<Bench>;

/* === Structs === */
#[derive(Clone, Copy, Debug)]
pub struct Bench;

pub struct Running {
    runtime: Runtime,
    probe: Option<harmos::services::ServiceLease<Bench>>,
}

/* === Enums === */
#[harmos::lanes]
pub enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Report,
}

/* === Traits === */

/* === Implementations === */
impl harmos::Application for Bench {
    type State = ();
    type Transaction = Never;
    type Record = ();
    type Streams = Streams;
    type Resources = Resources;
}

impl std::ops::Deref for Running {
    type Target = Runtime;
    fn deref(&self) -> &Runtime {
        &self.runtime
    }
}

impl std::fmt::Debug for Running {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Running").finish_non_exhaustive()
    }
}

impl Running {
    pub async fn stop(&mut self) -> harmos::Result<harmos::Position> {
        let position = self.runtime.stop().await?;
        self.probe.take();
        Ok(position)
    }
}

/* === Public Functions === */
pub fn builder(origin: impl Into<Origin>, resources: impl Into<Arc<Resources>>) -> Builder<Bench> {
    let resources = resources.into();
    harmos::Runtime::builder(origin, Arc::clone(&resources))
        .register(Streams::catalog())
        .operation_preparation_bytes(PREPARATION_BYTES)
        .register(Lanes::catalog())
        .operation::<Report>("minibench", Lanes::Report, resources)
}

#[tracing::instrument(level = "debug", skip_all, fields(device = %config.device()))]
pub async fn launch(config: &Config) -> Result<Running, Error> {
    let (resources, attachment) = Resources::connect(config)?;
    let runtime = builder((), Arc::new(resources)).runtime().await?;
    let mut running = Running {
        runtime,
        probe: None,
    };
    match probe::activate(&running.runtime, attachment).await {
        Ok(lease) => running.probe = Some(lease),
        Err(source) => {
            if let Err(cleanup) = running.stop().await {
                return Err(Error::Cleanup {
                    source: Box::new(cleanup),
                    activation: source,
                    running: Box::new(running),
                });
            }
            return Err(source.into());
        }
    }
    Ok(running)
}

#[cfg(feature = "sim")]
#[tracing::instrument(level = "debug", skip_all, fields(seed))]
pub fn simulate(origin: impl Into<Origin>, seed: u64) -> Result<Simulated, harmos::Incompatible> {
    builder(origin, Resources::simulated())
        .entropy_seed(seed)
        .simulate()
}

/* === Private Functions === */

/* === Tests === */
