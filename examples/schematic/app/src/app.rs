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
use std::ops::Deref;
use std::result::Result;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::config::{LaunchConfig, Settings};
use crate::document::Document;
use crate::error::Error;
use crate::records::{Records, records_registry};
use crate::state::Schematic;
use crate::storage;
use crate::streams;
use crate::transactions::{Transactions, transactions_state_catalog};
use harmos_runtime::plugin::manager::{self, Manager};
use harmos_runtime::prelude::*;
use harmos_runtime::storage::{StoreHistory, state::StorageStateSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Origin = harmos_runtime::state::Origin<Schematic>;

pub type Runtime = harmos_runtime::Runtime<SchematicEditor>;

pub type StateManager = harmos_runtime::state::StateManager<SchematicEditor>;

pub type StreamManager = harmos_runtime::streams::StreamManager<SchematicEditor>;

#[cfg(feature = "simulation")]
pub type Simulated = harmos_runtime::simulation::SimulatedRuntime<SchematicEditor>;

/* === Structs === */

pub struct SchematicEditor {
    checkpoint: Option<Arc<schematic_storage::Sidecar>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Lifecycle for SchematicEditor {
    type Config = Option<LaunchConfig>;
    type Error = std::io::Error;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(config: Self::Config, setup: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
        let checkpoint = if let Some(config) = config {
            let recovery = config.document.recovering()?;
            let checkpoint = Arc::new(recovery.sidecars());
            setup.resource::<schematic_storage::Sidecar>(Arc::clone(&checkpoint));
            storage::attach(setup, recovery, &config.settings)?;
            if config.plugins {
                let root = std::fs::canonicalize(config.settings.recordings())?;
                let history = StoreHistory {
                    store: Identifier::new("schematic-plugins").unwrap(),
                    epoch: Identifier::new("first").unwrap(),
                };
                let recovery = StorageStateSettings {
                    max_entry_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
                    max_checkpoint_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
                };
                let storage = crate::services::queries::attach(
                    setup,
                    &root.join("registry"),
                    history,
                    manager::catalog(),
                    config.settings.storage(),
                    recovery,
                )
                .map_err(std::io::Error::other)?;
                let settings = config.settings.plugins(root.join("plugins"));
                setup.resource(settings.clone());
                let manager = Manager::new(storage, settings).map_err(std::io::Error::other)?;
                setup.settings().plugins(manager.clone());
                setup.resource::<Manager>(manager);
            }
            Some(checkpoint)
        } else {
            None
        };

        let settings = harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(64 * 1024).unwrap()),
            ..Default::default()
        };
        setup
            .settings()
            .operation_settings(settings)
            .lanes(harmos_runtime::operations::LaneSet::new([crate::lanes::ANALYSIS]).map_err(std::io::Error::other)?);

        setup.settings().register(transactions_state_catalog()).register(records_registry()).register(streams::stream_definitions());
        setup.resource::<schematic_operations::schematic::upload::AnalysisResources>(Arc::new(Default::default()));
        setup.resource::<schematic_operations::transfer::Store>(Arc::new(Default::default()));
        Ok(Self { checkpoint })
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
}

/* === Public Functions === */

#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(
        journal = %document.journal().display(),
        generation = document.generation(),
    )
)]
pub async fn launch(document: Document, config: &Settings) -> Result<Arc<Runtime>, Error> {
    let origin = document.clone().into_origin();
    let config = LaunchConfig {
        document,
        settings: config.clone(),
        plugins: true,
    };
    let runtime = Arc::new(Runtime::launch(origin, Some(config)).await?);
    let manager = runtime.operations.deref().context().resource::<Manager>().map_err(std::io::Error::other)?;
    if let Err(error) = Box::pin(manager.start(&runtime)).await {
        runtime.shutdown().await.map_err(std::io::Error::other)?;
        return Err(std::io::Error::other(error).into());
    }
    Ok(runtime)
}

#[cfg(feature = "simulation")]
#[tracing::instrument(level = "debug", skip_all, fields(seed))]
pub fn simulate(origin: impl Into<Origin>, seed: u64) -> Result<Simulated, harmos_runtime::StartupError> {
    Runtime::simulate(origin, None, seed)
}

/* === Private Functions === */

/* === Tests === */
