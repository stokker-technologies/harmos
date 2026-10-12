/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "data/codec.rs"]
mod codec;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::SystemTime as Timestamp;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::storage::{SchemaKey, StorageCursor};
use crate::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordInput {
    pub schema: SchemaKey,
    pub scope: Option<Identifier>,
    pub item: Identifier,
    pub source: Option<Identifier>,
    pub source_time: Option<Timestamp>,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamInput {
    pub record: RecordInput,
    pub stream: Identifier,
    pub sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredRecord<T = Vec<u8>> {
    pub position: StorageCursor,
    pub schema: SchemaKey,
    pub scope: Option<Identifier>,
    pub item: Identifier,
    pub source: Option<Identifier>,
    #[serde(default, with = "crate::storage_timestamp::optional")]
    pub source_time: Option<Timestamp>,
    pub payload: T,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredRow<T = Vec<u8>> {
    pub record: StoredRecord<T>,
    pub stream: Identifier,
    pub sequence: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for RecordInput {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.schema.memory_size() - std::mem::size_of::<SchemaKey>()
            + self.scope.memory_size()
            - std::mem::size_of::<Option<Identifier>>()
            + self.item.memory_size()
            - std::mem::size_of::<Identifier>()
            + self.source.memory_size()
            - std::mem::size_of::<Option<Identifier>>()
            + self.payload.capacity()
    }
}
impl MemorySize for StreamInput {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.record.memory_size() - std::mem::size_of::<RecordInput>()
            + self.stream.memory_size()
            - std::mem::size_of::<Identifier>()
    }
}
#[cfg(feature = "runtime")]
impl RecordInput {
    pub fn borrowed(&self) -> crate::storage::RecordWrite<'_> {
        crate::storage::RecordWrite {
            schema: &self.schema,
            scope: self.scope.as_ref(),
            item: &self.item,
            source: self.source.as_ref(),
            source_time: self.source_time,
            payload: &self.payload,
        }
    }
}
#[cfg(feature = "runtime")]
impl StreamInput {
    pub fn borrowed(&self) -> crate::storage::StreamWrite<'_> {
        crate::storage::StreamWrite {
            record: self.record.borrowed(),
            stream: &self.stream,
            sequence: self.sequence,
        }
    }
}

impl<T: MemorySize> MemorySize for StoredRecord<T> {
    fn memory_size(&self) -> usize {
        let dynamic = [
            self.position
                .memory_size()
                .saturating_sub(std::mem::size_of::<StorageCursor>()),
            self.schema
                .memory_size()
                .saturating_sub(std::mem::size_of::<SchemaKey>()),
            self.scope
                .memory_size()
                .saturating_sub(std::mem::size_of::<Option<Identifier>>()),
            self.item
                .memory_size()
                .saturating_sub(std::mem::size_of::<Identifier>()),
            self.source
                .memory_size()
                .saturating_sub(std::mem::size_of::<Option<Identifier>>()),
            self.payload
                .memory_size()
                .saturating_sub(std::mem::size_of::<T>()),
        ];
        dynamic
            .into_iter()
            .fold(std::mem::size_of::<Self>(), usize::saturating_add)
    }
}

impl<T: MemorySize> MemorySize for StoredRow<T> {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.stream
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<Identifier>()),
            )
            .saturating_add(
                self.record
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StoredRecord<T>>()),
            )
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
