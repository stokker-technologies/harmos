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
use std::result::Result;

// Third Party Imports

// Local Imports
use crate::config::Settings;
use crate::document::Document;
use crate::error::Error;
use crate::records::Records;
use crate::state::Schematic;
use crate::storage;
use crate::streams::Streams;
use crate::transactions::Transactions;
use harmos::prelude::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Origin = harmos::state::Origin<Schematic>;

pub type Runtime = harmos::Runtime<SchematicEditor>;

pub type StateManager = harmos::state::StateManager<SchematicEditor>;

pub type StreamManager = harmos::streams::StreamManager<SchematicEditor>;

#[cfg(feature = "simulation")]
pub type Simulated = harmos::simulation::SimulatedRuntime<SchematicEditor>;

/* === Structs === */

#[derive(Clone, Copy, Debug)]
pub struct SchematicEditor;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Application for SchematicEditor {
    type State = Schematic;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = Streams;
    type Resources = ();
    type Lanes = crate::lanes::Lanes;
}

/* === Public Functions === */

pub fn builder(origin: impl Into<Origin>, resources: ()) -> Builder<SchematicEditor> {
    let settings = harmos::operations::Settings {
        preparation_bytes: Some(NonZeroUsize::new(64 * 1024).unwrap()),
        ..Default::default()
    };
    harmos::Runtime::builder(origin, resources)
        .register(Transactions::catalog())
        .register(Records::catalog())
        .register(Streams::catalog())
        .register(crate::lanes::Lanes::catalog())
        .operation_settings(settings)
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
    let recovery = document.recovering()?;
    let origin = document.into_origin();
    let assembly = builder(origin, ());
    let assembly = storage::attach(assembly, recovery, config)?;

    Ok(assembly.runtime().await?)
}

#[cfg(feature = "simulation")]
#[tracing::instrument(level = "debug", skip_all, fields(seed))]
pub fn simulate(origin: impl Into<Origin>, seed: u64) -> Result<Simulated, harmos::runtime::Incompatible> {
    builder(origin, ()).entropy_seed(seed).simulate()
}

/* === Private Functions === */

/* === Tests === */
