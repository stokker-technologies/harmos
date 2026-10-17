/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State Backend Resource Limits

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
/// Consumer-owned limits for one durable state root, measured in encoded bytes
/// written to disk, including each record's integrity envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageStateSettings {
    /// Largest encoded journal record for one recovery entry.
    pub max_entry_bytes: NonZeroUsize,
    /// Largest encoded checkpoint record.
    pub max_checkpoint_bytes: NonZeroUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
