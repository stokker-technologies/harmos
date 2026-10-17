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
use std::time::SystemTime as Timestamp;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::model::storage::{SchemaKey, StorageCursor};
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
    #[serde(default, with = "crate::model::storage::time::optional")]
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

#[derive(Clone, Copy, Debug, Serialize)]
pub struct RecordWrite<'a> {
    pub schema: &'a SchemaKey,
    pub scope: Option<&'a Identifier>,
    pub item: &'a Identifier,
    pub source: Option<&'a Identifier>,
    pub source_time: Option<Timestamp>,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct StreamWrite<'a> {
    pub stream: &'a Identifier,
    pub sequence: u64,
    pub record: RecordWrite<'a>,
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

impl RecordInput {
    pub fn borrowed(&self) -> RecordWrite<'_> {
        RecordWrite {
            schema: &self.schema,
            scope: self.scope.as_ref(),
            item: &self.item,
            source: self.source.as_ref(),
            source_time: self.source_time,
            payload: &self.payload,
        }
    }
}

impl StreamInput {
    pub fn borrowed(&self) -> StreamWrite<'_> {
        StreamWrite {
            record: self.record.borrowed(),
            stream: &self.stream,
            sequence: self.sequence,
        }
    }
}

impl RecordWrite<'_> {
    pub fn stored(&self, position: StorageCursor) -> StoredRecord {
        StoredRecord {
            position,
            schema: self.schema.clone(),
            scope: self.scope.cloned(),
            item: self.item.clone(),
            source: self.source.cloned(),
            source_time: self.source_time,
            payload: self.payload.to_vec(),
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
