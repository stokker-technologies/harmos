/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


/* === Modules === */
pub mod __harmos {
    pub use harmos::{artifact, catalog, listeners, operations, schema};
    pub use harmos::{Batched, Definition, MemorySize, Message, MessageError, Metadata, RowMetadata, StreamField, StreamSpec, __private, guest, sidecar, Capacity, Lane, LaneSet, Scope};
}

/* === Re-Exports === */
pub use serde::*;
pub use harmos::{stream, message, plugin, observer, listener, catalog, lanes};

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::stream(id = "reading", window = 8)]
#[harmos::message]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Reading { pub value: u32 }

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
