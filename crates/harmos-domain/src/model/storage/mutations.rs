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

crate::model::storage::codec::product!(MutationId {
    window: crate::model::storage::codec::Id => 1,
    key: crate::model::storage::codec::Id => 2,
});
crate::model::storage::codec::product!(MutationWindow {
    history: crate::model::storage::codec::Nested<StoreHistory> => 1,
    id: crate::model::storage::codec::Id => 2,
    expires_at: crate::model::storage::codec::Time => 3,
});
crate::model::storage::codec::product!(FileProducer {
    session: crate::model::storage::codec::Nested<FileSession> => 1,
    length: crate::model::storage::codec::Unsigned => 2,
});

crate::model::storage::codec::product!(StorageInfo {
    accepted: crate::model::storage::codec::Nested<StorageCursor> => 1,
    saved: crate::model::storage::codec::Nested<StorageCursor> => 2,
    mutations: crate::model::storage::codec::Optional<crate::model::storage::codec::Nested<MutationWindow>> => 3,
    file_batch_bytes: crate::model::storage::codec::Unsigned => 4,
});

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
