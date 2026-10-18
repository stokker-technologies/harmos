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
use crate::state::NetId;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::stream(
    description = "One low-rate voltage sample from the editor's simulator.\nThe runtime witnesses the journal position at publish. The time belongs in the payload because it is part of what this irregular feed declares.",
    id = "net_voltage",
    window = 4
)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct NetVoltage {
    pub net: NetId,
    pub sampled_at_us: u64,
    pub millivolts: i32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl NetVoltage {
    pub const fn teaching(sampled_at_us: u64, millivolts: i32) -> Self {
        Self { net: NetId(1), sampled_at_us, millivolts }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
