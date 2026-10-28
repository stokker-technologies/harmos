/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::num::NonZeroUsize;
use std::result::Result;
use std::sync::Arc;

use crate::config::{LaunchConfig, Settings};
use crate::document::Document;
use crate::error::Error;
use crate::records::Records;
use crate::state::Schematic;
use crate::storage;
use crate::streams::Streams;
use crate::transactions::Transactions;
use harmos::prelude::*;

pub type Origin = harmos::state::Origin<Schematic>;

pub type Runtime = harmos::Runtime<SchematicEditor>;

pub type StateManager = harmos::state::StateManager<SchematicEditor>;

pub type StreamManager = harmos::streams::StreamManager<SchematicEditor>;

#[cfg(feature = "simulation")]
pub type Simulated = harmos::simulation::SimulatedRuntime<SchematicEditor>;

pub struct SchematicEditor {
    checkpoint: Option<Arc<schematic_storage::Sidecar>>,
}

impl Lifecycle for SchematicEditor {
    type Config = Option<LaunchConfig>;
    type Error = std::io::Error;
    type Context = harmos::Startup<Self>;

    async fn prepare(config: Self::Config, setup: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> {
        // Resources
        let checkpoint = if let Some(config) = config {
            let recovery = config.document.recovering()?;
            let checkpoint = Arc::new(recovery.sidecars());
            setup.resource::<schematic_storage::Sidecar>(Arc::clone(&checkpoint));
            storage::attach(setup, recovery, &config.settings)?;
            Some(checkpoint)
        } else {
            None
        };

        // Lanes
        let settings = harmos::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(64 * 1024).unwrap()),
            ..Default::default()
        };
        setup.settings().operation_settings(settings);

        // Operations
        setup.settings().register(Transactions::catalog()).register(Records::catalog()).register(Streams::catalog());
        Ok(Self { checkpoint })
    }

    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.checkpoint.take();
        Ok(())
    }
}

impl Application for SchematicEditor {
    type State = Schematic;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = Streams;
    type Lanes = crate::lanes::Lanes;
}

#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(
        journal = %document.journal().display(),
        generation = document.generation(),
    )
)]
pub async fn launch(document: Document, config: &Settings) -> Result<Runtime, Error> {
    let origin = document.clone().into_origin();
    let config = LaunchConfig { document, settings: config.clone() };
    let runtime = Runtime::launch(origin, Some(config)).await?;
    Ok(runtime)
}

#[cfg(feature = "simulation")]
#[tracing::instrument(level = "debug", skip_all, fields(seed))]
pub fn simulate(origin: impl Into<Origin>, seed: u64) -> Result<Simulated, harmos::StartupError> {
    Runtime::simulate(origin, None, seed)
}
