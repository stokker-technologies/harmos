/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod codec;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreHistory {
    pub store: Identifier,
    pub epoch: Identifier,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageCursor {
    pub history: StoreHistory,
    pub sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryPosition {
    pub history: StoreHistory,
    pub sequence: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for StoreHistory {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.store.memory_size().saturating_sub(std::mem::size_of::<Identifier>()))
            .saturating_add(self.epoch.memory_size().saturating_sub(std::mem::size_of::<Identifier>()))
    }
}

impl MemorySize for StorageCursor {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.history.memory_size().saturating_sub(std::mem::size_of::<StoreHistory>()))
    }
}

impl StoreHistory {
    pub fn check(&self, other: &Self) -> Result<()> {
        if self == other { Ok(()) } else { Err(Error::DifferentHistory) }
    }
}

impl StorageCursor {
    pub fn covers(&self, other: &Self) -> Result<bool> {
        self.history.check(&other.history)?;
        Ok(self.sequence >= other.sequence)
    }
}

impl RecoveryPosition {
    pub fn covers(&self, other: &Self) -> Result<bool> {
        self.history.check(&other.history)?;
        Ok(self.sequence >= other.sequence)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
