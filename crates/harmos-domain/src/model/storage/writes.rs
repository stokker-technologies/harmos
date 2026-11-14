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
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::ArrowIpc;
use super::{SchemaKey, StorageCursor};
use crate::schema::graph::Schema;
use crate::{Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendSnapshot {
    pub at: StorageCursor,
    pub token: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReadLimits {
    pub rows: NonZeroUsize,
    pub bytes: NonZeroUsize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageSchema {
    pub position: StorageCursor,
    pub key: SchemaKey,
    pub schema: Arc<Schema>,
}

#[derive(Clone, Debug)]
pub struct DataBatch {
    pub sequence: u64,
    pub destination: Identifier,
    pub schema: SchemaKey,
    pub data: ArrowIpc,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ReadLimits {
    pub fn check(&self, rows: usize, bytes: usize) -> Result<()> {
        if rows > self.rows.get() || bytes > self.bytes.get() {
            return Err(crate::Error::InvalidInput {
                field: "read page",
                reason: "row or byte bound exceeded",
            });
        }
        Ok(())
    }
}

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
}

impl MemorySize for DataBatch {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.destination.memory_size().saturating_sub(std::mem::size_of_val(&self.destination)))
            .saturating_add(self.schema.memory_size().saturating_sub(std::mem::size_of_val(&self.schema)))
            .saturating_add(self.data.memory_size().saturating_sub(std::mem::size_of_val(&self.data)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

impl crate::MemorySize for ReadLimits {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
