/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod error;
pub mod evidence;
pub mod journal;
pub mod snapshot;
pub mod streams;

/* === Re-Exports === */

pub use error::{Error, Result};
pub use evidence::EvidenceDirectory;
pub use journal::{Jsonl, Sidecar, Wire};
pub use snapshot::Snapshot;
pub use streams::JsonlStreams;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
