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

#[derive(harmos_runtime::Row, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct NetVoltage {
    pub net: NetId,
    pub sampled_at_us: u64,
    pub voltage_mv: i32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::streams::StreamRow for NetVoltage {
    const ENTRY: harmos_runtime::StreamEntry<Self> = harmos_runtime::StreamEntry::new("net_voltage", 4).label("Net Voltage").description("One low-rate voltage sample from the editor's simulator.\nThe runtime witnesses the journal position at publish. The time belongs in the payload because it is part of what this irregular feed declares.");
}

impl NetVoltage {
    pub const fn teaching(sampled_at_us: u64, voltage_mv: i32) -> Self {
        Self { net: NetId(1), sampled_at_us, voltage_mv }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
