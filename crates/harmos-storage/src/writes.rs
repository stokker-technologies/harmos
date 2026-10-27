/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod convert;
mod decoded;

/* === Re-Exports === */

pub use decoded::DecodedBatch;
pub use harmos_domain::model::storage::{BatchReceipt, DataBatch, WriteCompletion, WriteReceipt};

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
// Local Imports
use super::manager::ENTRY_OVERHEAD;
use super::{SchemaKey, StorageCursor, StoreHistory};
use crate::schema::graph::Schema;
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageSchema {
    pub position: StorageCursor,
    pub key: SchemaKey,
    pub schema: Arc<Schema>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for StorageSchema {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.position.memory_size().saturating_sub(std::mem::size_of::<StorageCursor>()))
            .saturating_add(self.key.memory_size().saturating_sub(std::mem::size_of::<SchemaKey>()))
            .saturating_add(self.schema.memory_size().saturating_add(2 * std::mem::size_of::<usize>()))
    }
}

impl StorageSchema {
    pub fn new(position: StorageCursor, key: SchemaKey, schema: impl Into<Arc<Schema>>) -> Self {
        Self { position, key, schema: schema.into() }
    }

    pub(super) fn resident_bytes(key: &SchemaKey, schema: &Schema, history: &StoreHistory) -> Result<usize> {
        let allocations = schema.memory_size().checked_add(2 * std::mem::size_of::<usize>()).ok_or_else(size_error)?;
        [
            std::mem::size_of::<Self>(),
            ENTRY_OVERHEAD,
            key.id.as_str().len(),
            history.store.as_str().len(),
            history.epoch.as_str().len(),
            allocations,
        ]
        .into_iter()
        .try_fold(0usize, |bytes, next| bytes.checked_add(next))
        .ok_or_else(size_error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn size_error() -> Error {
    Error::InvalidInput {
        field: "storage schema",
        reason: "size overflow",
    }
}

/* === Tests === */
