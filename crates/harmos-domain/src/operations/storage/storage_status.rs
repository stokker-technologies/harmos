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

// Local Imports
use super::POLICY;
use crate::MemorySize;
use crate::model::storage::StorageInfo;
use crate::model::storage::codec::product;
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(StorageStatus {});

crate::declare!(StorageStatus {
    id: "storage.status",
    version: 1,
    policy: POLICY,
    response: StorageInfo,
    visibility: Visibility::Host,
    replay: ReplaySafety::Idempotent,
});

/* === Type Aliases === */

/* === Structs === */

// Read The Storage Frontiers, Mutation Window And File Batch Size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageStatus {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for StorageStatus {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
