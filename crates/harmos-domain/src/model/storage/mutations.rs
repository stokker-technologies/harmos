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

impl From<MutationWindow> for harmos_protocol::storage::MutationWindow {
    fn from(value: MutationWindow) -> Self {
        Self {
            history: Some(value.history.into()),
            id: value.id.to_string(),
            expires_at: Some(value.expires_at.into()),
        }
    }
}

impl From<StorageInfo> for harmos_protocol::storage::StorageInfo {
    fn from(value: StorageInfo) -> Self {
        Self {
            accepted: Some(value.accepted.into()),
            saved: Some(value.saved.into()),
            mutations: value.mutations.map(Into::into),
            file_batch_bytes: value.file_batch_bytes,
        }
    }
}

impl From<MutationId> for harmos_protocol::storage::MutationId {
    fn from(value: MutationId) -> Self {
        Self {
            window: value.window.to_string(),
            key: value.key.to_string(),
        }
    }
}

impl TryFrom<harmos_protocol::storage::MutationId> for MutationId {
    type Error = crate::Error;
    fn try_from(value: harmos_protocol::storage::MutationId) -> crate::Result<Self> {
        Ok(Self {
            window: Identifier::new(value.window)?,
            key: Identifier::new(value.key)?,
        })
    }
}

impl From<FileProducer> for harmos_protocol::storage::FileProducer {
    fn from(value: FileProducer) -> Self {
        Self {
            session: Some(value.session.into()),
            length: value.length,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
