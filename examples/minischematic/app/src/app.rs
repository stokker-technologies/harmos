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

// Third Party Imports

// Local Imports
use crate::config::Config;
use crate::document::Document;
use crate::error::Error;
use crate::records::Records;
use crate::state::Schematic;
use crate::storage;
use crate::streams::Streams;
use crate::transactions::Transactions;
use harmos::Builder;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type Origin = harmos::Origin<Schematic>;

pub type Runtime = harmos::Runtime<SchematicEditor>;

pub type StateManager = harmos::StateManager<SchematicEditor>;

pub type StreamManager = harmos::StreamManager<SchematicEditor>;

#[cfg(feature = "sim")]
pub type Simulated = harmos::SimulatedRuntime<SchematicEditor>;

/* === Structs === */
#[derive(Clone, Copy, Debug)]
pub struct SchematicEditor;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::Application for SchematicEditor {
    type State = Schematic;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = Streams;
    type Resources = ();
}

/* === Public Functions === */
pub fn builder(origin: impl Into<Origin>, resources: ()) -> Builder<SchematicEditor> {
    harmos::Runtime::builder(origin, resources)
        .register(Transactions::catalog())
        .register(Records::catalog())
        .register(Streams::catalog())
        .register(crate::services::autoroute::Lanes::catalog())
        .operation_preparation_bytes(
            NonZeroUsize::new(2 * minischematic_contract::worker::PAYLOAD_BYTES + 4096).unwrap(),
        )
}

#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(
        sidecar = %document.sidecar().display(),
        generation = document.generation(),
    )
)]
pub async fn launch(document: Document, config: &Config) -> Result<Runtime, Error> {
    let recovery = document.recovering()?;
    let assembly = storage::attach(builder(document.into_origin(), ()), recovery, config)?;

    Ok(assembly.runtime().await?)
}

#[cfg(feature = "sim")]
#[tracing::instrument(level = "debug", skip_all, fields(seed))]
pub fn simulate(origin: impl Into<Origin>, seed: u64) -> Result<Simulated, harmos::Incompatible> {
    builder(origin, ()).entropy_seed(seed).simulate()
}

/* === Private Functions === */

/* === Tests === */
