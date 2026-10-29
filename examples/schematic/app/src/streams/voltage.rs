/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use serde::{Deserialize, Serialize};

use crate::state::NetId;

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

impl NetVoltage {
    pub const fn teaching(sampled_at_us: u64, millivolts: i32) -> Self {
        Self { net: NetId(1), sampled_at_us, millivolts }
    }
}
