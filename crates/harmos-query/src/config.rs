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

use serde::{Deserialize, Serialize};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryAccumulatorSettings {
    pub memory_bytes: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Default for QueryAccumulatorSettings {
    fn default() -> Self {
        Self { memory_bytes: 64 * 1024 }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
