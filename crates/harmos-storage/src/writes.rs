/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub mod convert;
mod decoded;

pub use decoded::DecodedBatch;
pub use harmos_domain::model::storage::{BatchReceipt, DataBatch, WriteCompletion, WriteReceipt};

use std::sync::Arc;

use super::{SchemaKey, StorageCursor};
use crate::MemorySize;
use crate::schema::graph::Schema;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageSchema {
    pub position: StorageCursor,
    pub key: SchemaKey,
    pub schema: Arc<Schema>,
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
