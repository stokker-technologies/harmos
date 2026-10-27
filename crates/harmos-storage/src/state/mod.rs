/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod admission;
pub mod config;
pub mod file;
mod reader;

/* === Re-Exports === */

pub use admission::Permit;
pub use config::StorageStateSettings;
pub use reader::RecoveryReader;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::{ReadLimits, RecoveryPosition, StorageBatch, StorageFuture, StoreHistory};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryEntry {
    pub required_data: super::StorageCursor,
    pub position: RecoveryPosition,

    #[serde(with = "payload")]
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverySnapshot {
    pub required_data: super::StorageCursor,
    pub through: RecoveryPosition,

    #[serde(with = "payload")]
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryPage {
    pub entries: Vec<RecoveryEntry>,
    pub next: Option<RecoveryPosition>,
}

/* === Enums === */

/* === Traits === */

pub trait StateBackend: Send + Sync + 'static {
    fn history(&self) -> &StoreHistory;
    fn open(&self) -> StorageFuture<'_, RecoveryPosition>;
    fn read_snapshot(&self, max_bytes: std::num::NonZeroUsize) -> StorageFuture<'_, Option<RecoverySnapshot>>;
    fn write_snapshot(&self, snapshot: StorageBatch<RecoverySnapshot>) -> StorageFuture<'_, ()>;
    fn read_entries<'a>(
        &'a self, after: &'a RecoveryPosition, through: &'a RecoveryPosition, limits: ReadLimits,
    ) -> StorageFuture<'a, RecoveryPage>;
    fn write_entries(&self, entries: StorageBatch<RecoveryEntry>) -> StorageFuture<'_, ()>;
    fn flush<'a>(&'a self, through: &'a RecoveryPosition) -> StorageFuture<'a, RecoveryPosition>;
    fn close(&self) -> StorageFuture<'_, ()>;
}

/* === Implementations === */

impl MemorySize for RecoveryEntry {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.required_data.memory_size().saturating_sub(std::mem::size_of::<super::StorageCursor>()))
            .saturating_add(self.position.history.memory_size().saturating_sub(std::mem::size_of::<StoreHistory>()))
            .saturating_add(self.payload.capacity())
    }
}

impl MemorySize for RecoverySnapshot {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.required_data.memory_size().saturating_sub(std::mem::size_of::<super::StorageCursor>()))
            .saturating_add(self.through.history.memory_size().saturating_sub(std::mem::size_of::<StoreHistory>()))
            .saturating_add(self.payload.capacity())
    }
}

impl MemorySize for RecoveryPage {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.entries.memory_size().saturating_sub(std::mem::size_of::<Vec<RecoveryEntry>>()))
            .saturating_add(
                self.next.as_ref().map_or(0, |next| next.history.memory_size().saturating_sub(std::mem::size_of::<StoreHistory>())),
            )
    }
}

impl RecoveryPage {
    pub fn check(
        &self, after: &RecoveryPosition, through: &RecoveryPosition, limits: ReadLimits,
        mut encoded_bytes: impl FnMut(&RecoveryEntry) -> usize,
    ) -> Result<()> {
        if !through.covers(after)? {
            return Err(Error::InvalidInput { field: "recovery range", reason: "start exceeds fixed frontier" });
        }
        let mut previous = after.sequence;
        let mut bytes = 0usize;

        for entry in &self.entries {
            after.history.check(&entry.position.history)?;
            let expected = previous.checked_add(1).ok_or(Error::InvalidInput { field: "recovery page", reason: "position overflow" })?;
            if entry.position.sequence < expected {
                return Err(Error::InvalidInput { field: "recovery page", reason: "duplicate or reversed position" });
            }
            if entry.position.sequence > expected {
                return Err(Error::Gap { requested: expected, available: entry.position.sequence });
            }
            if entry.position.sequence > through.sequence {
                return Err(Error::InvalidInput { field: "recovery page", reason: "entry exceeds fixed frontier" });
            }
            bytes = bytes
                .checked_add(encoded_bytes(entry))
                .ok_or(Error::InvalidInput { field: "recovery page", reason: "encoded size overflow" })?;
            previous = entry.position.sequence;
        }
        limits.check(self.entries.len(), bytes)?;

        let expected_next =
            if previous < through.sequence { Some(RecoveryPosition { history: after.history.clone(), sequence: previous }) } else { None };
        if self.next != expected_next || (self.entries.is_empty() && expected_next.is_some()) {
            return Err(Error::InvalidInput { field: "recovery page", reason: "missing progress or invalid continuation" });
        }

        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

// Opaque Payloads Encode As Unpadded Base64, So Every Byte Changes The Encoded Size.
mod payload {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD_NO_PAD;
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub(super) fn serialize<S: Serializer>(payload: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD_NO_PAD.encode(payload))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        STANDARD_NO_PAD.decode(encoded.as_bytes()).map_err(D::Error::custom)
    }
}

/* === Tests === */
