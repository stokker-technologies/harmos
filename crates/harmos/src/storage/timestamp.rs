/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(super) mod optional {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &Option<Timestamp>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Wire::from).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Timestamp>, D::Error> {
        Option::<Wire>::deserialize(deserializer)?
            .map(Wire::timestamp)
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::time::{Duration, SystemTime as Timestamp, UNIX_EPOCH};
// Third Party Imports
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Wire {
    pub(crate) secs_since_epoch: u64,
    pub(crate) nanos_since_epoch: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) before_epoch: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl From<Timestamp> for Wire {
    fn from(value: Timestamp) -> Self {
        let (duration, before_epoch) = match value.duration_since(UNIX_EPOCH) {
            Ok(duration) => (duration, false),
            Err(error) => (error.duration(), true),
        };

        Self {
            secs_since_epoch: duration.as_secs(),
            nanos_since_epoch: duration.subsec_nanos(),
            before_epoch,
        }
    }
}

impl Wire {
    pub(crate) fn timestamp(self) -> Result<Timestamp, &'static str> {
        if self.nanos_since_epoch >= 1_000_000_000 {
            return Err("invalid source timestamp nanoseconds");
        }
        let duration = Duration::new(self.secs_since_epoch, self.nanos_since_epoch);
        let timestamp = if self.before_epoch {
            if duration.is_zero() {
                return Err("invalid negative zero timestamp");
            }
            UNIX_EPOCH.checked_sub(duration)
        } else {
            UNIX_EPOCH.checked_add(duration)
        };

        timestamp.ok_or("source timestamp outside platform range")
    }
}

/* === Public Functions === */
pub(super) fn serialize<S: Serializer>(
    value: &Timestamp,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    Wire::from(*value).serialize(serializer)
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Timestamp, D::Error> {
    Wire::deserialize(deserializer)?
        .timestamp()
        .map_err(serde::de::Error::custom)
}

/* === Private Functions === */
fn is_false(value: &bool) -> bool {
    !*value
}

/* === Tests === */
