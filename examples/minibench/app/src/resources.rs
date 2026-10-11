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
use std::collections::BTreeMap;
// Third Party Imports

// Local Imports
use crate::config::Config;
use crate::error::ProbeError;
use crate::services::probe::{self, Attachment};

/* === Constants === */
pub const SIMULATED: &str = "sim://simulated";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, Default)]
pub struct Calibration {
    offsets: BTreeMap<u32, i32>,
}

pub struct Resources {
    pub device: String,
    pub calibration: Calibration,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Calibration {
    pub fn measured(offsets: impl IntoIterator<Item = (u32, i32)>) -> Self {
        Self {
            offsets: offsets.into_iter().collect(),
        }
    }

    pub fn correct(&self, channel: u32, millivolts: i32) -> i32 {
        millivolts - self.offsets.get(&channel).copied().unwrap_or_default()
    }
}

impl Resources {
    #[tracing::instrument(level = "debug", skip_all, fields(device = %config.device()))]
    pub fn connect(config: &Config) -> Result<(Self, Attachment), ProbeError> {
        let attachment = probe::attach(config)?;

        Ok((
            Self {
                device: config.device().to_owned(),
                calibration: Calibration::measured([(1, 10), (2, 10), (3, 10)]),
            },
            attachment,
        ))
    }

    pub fn simulated() -> Self {
        Self {
            device: SIMULATED.to_owned(),
            calibration: Calibration::default(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
