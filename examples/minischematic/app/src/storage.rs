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
use crate::config::Config;
use crate::streams::NetVoltage;
use harmos::Builder;
use jsonl_adapter::{Jsonl, JsonlStreams};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn recordings(
    assembly: Builder<SchematicEditor>,
    config: &Config,
) -> Result<Builder<SchematicEditor>, io::Error> {
    Ok(assembly.attach_streams(recording(config)?))
}

pub fn attach(
    assembly: Builder<SchematicEditor>,
    recovery: Jsonl<SchematicEditor>,
    config: &Config,
) -> Result<Builder<SchematicEditor>, io::Error> {
    let recovery = recovery.syncing_every(config.journal_sync_entries.get());
    recordings(assembly.recover(recovery), config)
}

/* === Private Functions === */
fn recording(config: &Config) -> Result<JsonlStreams<SchematicEditor>, io::Error> {
    Ok(JsonlStreams::<SchematicEditor>::open(config.recordings())?.select::<NetVoltage>())
}

/* === Tests === */
