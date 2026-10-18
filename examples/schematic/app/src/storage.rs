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
use std::io;

// Third Party Imports

// Local Imports
use crate::app::SchematicEditor;
use crate::config::Settings;
use crate::streams::NetVoltage;
use harmos::Builder;
use schematic_storage::{Jsonl, JsonlStreams};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn recordings(assembly: Builder<SchematicEditor>, config: &Settings) -> Result<Builder<SchematicEditor>, io::Error> {
    let recordings = JsonlStreams::<SchematicEditor>::open(config.recordings())?;
    let recordings = recordings.select::<NetVoltage>();
    Ok(assembly.attach_streams(recordings))
}

pub fn attach(
    assembly: Builder<SchematicEditor>, recovery: Jsonl<SchematicEditor>, config: &Settings,
) -> Result<Builder<SchematicEditor>, io::Error> {
    let recovery = recovery.syncing_every(config.journal_sync_entries.get());
    let assembly = assembly.recover(recovery);
    recordings(assembly, config)
}

/* === Private Functions === */
/* === Tests === */
