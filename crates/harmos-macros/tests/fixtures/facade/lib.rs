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
    pub use harmos::{catalog, guest, listeners, operations, plugin, schema, sidecar, streams};
    pub use harmos::{MemorySize, Message, MessageError, Metadata, RowMetadata, StreamField, StreamSpec, __private};
}

/* === Re-Exports === */
pub use serde::*;
pub use harmos::{stream, message, plugin, observer, listener, catalog, lanes, operation};

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
