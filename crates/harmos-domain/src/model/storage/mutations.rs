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
use std::time::SystemTime;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::model::storage::{FileSession, StorageCursor, StoreHistory};
use crate::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationId {
    pub window: Identifier,
    pub key: Identifier,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationWindow {
    pub history: StoreHistory,
    pub id: Identifier,
    pub expires_at: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileProducer {
    pub session: FileSession,
    pub length: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageInfo {
    pub accepted: StorageCursor,
    pub saved: StorageCursor,
    pub mutations: Option<MutationWindow>,
    pub file_batch_bytes: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for MutationId {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.window.memory_size() - std::mem::size_of::<Identifier>() + self.key.memory_size()
            - std::mem::size_of::<Identifier>()
    }
}
impl MemorySize for MutationWindow {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.history.memory_size() - std::mem::size_of::<StoreHistory>() + self.id.memory_size()
            - std::mem::size_of::<Identifier>()
    }
}
impl MemorySize for FileProducer {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.session.memory_size() - std::mem::size_of::<FileSession>()
    }
}

impl MemorySize for StorageInfo {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.accepted.memory_size() - std::mem::size_of::<StorageCursor>() + self.saved.memory_size()
            - std::mem::size_of::<StorageCursor>()
            + self.mutations.memory_size()
            - std::mem::size_of::<Option<MutationWindow>>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
