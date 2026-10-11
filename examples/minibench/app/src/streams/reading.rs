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

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::stream(
    description = "One reading published by the probe sidecar.",
    id = "reading",
    window = 32,
    batch(samples = 3, gaps = "report")
)]
#[harmos::message]
#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub struct Reading {
    pub device: String,

    pub channel: u32,

    #[harmos(measure)]
    pub millivolts: i32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
