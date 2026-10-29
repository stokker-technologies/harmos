/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub use schematic_storage::Sidecar;

use std::io;

use crate::app::SchematicEditor;
use crate::config::Settings;
use crate::streams::NetVoltage;
use harmos::Setup;
use schematic_storage::{Jsonl, JsonlStreams};

pub fn recordings(setup: &mut Setup<SchematicEditor>, config: &Settings) -> Result<(), io::Error> {
    let recordings = JsonlStreams::<SchematicEditor>::open(config.recordings())?;
    let recordings = recordings.select::<NetVoltage>();
    setup.settings().attach_streams(recordings);
    Ok(())
}

pub fn attach(setup: &mut Setup<SchematicEditor>, recovery: Jsonl<SchematicEditor>, config: &Settings) -> Result<(), io::Error> {
    let recovery = recovery.syncing_every(config.journal_sync_entries.get());
    setup.settings().recover(recovery);
    recordings(setup, config)
}
