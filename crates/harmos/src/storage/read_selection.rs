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
use std::time::SystemTime as Timestamp;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::object_catalog::{ObjectKey, SchemaKey};
use crate::storage_codec::{Code, Count, Id, Nested, Optional, Time, Unsigned, coded, product};
use crate::storage_object_values::Key;
use crate::storage_object_values::ObjectVersions;
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTimeRange {
    #[serde(with = "crate::storage_timestamp")]
    pub start: Timestamp,
    #[serde(with = "crate::storage_timestamp")]
    pub end: Timestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadLimits {
    pub rows: NonZeroUsize,
    pub bytes: NonZeroUsize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadSelection {
    pub kind: ReadKind,
    pub schema: Option<SchemaKey>,
    pub scope: Option<Identifier>,
    pub stream: Option<Identifier>,
    pub item: Option<Identifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_key: Option<ObjectKey>,
    #[serde(default, skip_serializing_if = "ObjectVersions::is_revisions")]
    pub object_versions: ObjectVersions,
    pub after: u64,
    pub through: Option<u64>,
    pub order: ReadOrder,
    pub history: ReadHistory,
    pub source_time: Option<SourceTimeRange>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadKind {
    Objects,
    Schemas,
    Records,
    Stream,
    Files,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadHistory {
    Complete,
    Retained,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadOrder {
    Ascending,
    Descending,
}

/* === Traits === */

/* === Implementations === */
impl SourceTimeRange {
    pub fn contains(&self, time: Option<Timestamp>) -> bool {
        time.is_some_and(|time| time >= self.start && time < self.end)
    }
}

impl MemorySize for ReadSelection {
    fn memory_size(&self) -> usize {
        let mut bytes = std::mem::size_of::<Self>();
        for identifier in [
            self.schema.as_ref().map(|schema| &schema.id),
            self.scope.as_ref(),
            self.stream.as_ref(),
            self.item.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            bytes = bytes.saturating_add(
                identifier
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<Identifier>()),
            );
        }
        if let Some(key) = &self.object_key {
            bytes = bytes.saturating_add(
                key.memory_size()
                    .saturating_sub(std::mem::size_of::<ObjectKey>()),
            );
        }
        bytes
    }
}

impl ReadSelection {
    pub fn new(kind: ReadKind) -> Self {
        Self {
            kind,
            schema: None,
            scope: None,
            stream: None,
            item: None,
            object_key: None,
            object_versions: ObjectVersions::Revisions,
            after: 0,
            through: None,
            order: ReadOrder::Ascending,
            history: ReadHistory::Complete,
            source_time: None,
        }
    }
}

impl ReadLimits {
    pub fn check(&self, rows: usize, bytes: usize) -> Result<()> {
        if rows > self.rows.get() || bytes > self.bytes.get() {
            return Err(Error::InvalidInput {
                field: "read page",
                reason: "row or byte bound exceeded",
            });
        }

        Ok(())
    }
}

impl MemorySize for SourceTimeRange {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl MemorySize for ReadLimits {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

// Wire Codecs Shared By Plugins, Frames And The Runtime Reader.
coded!(ReadKind {
    Objects,
    Schemas,
    Records,
    Stream,
    Files
});
coded!(ReadHistory { Complete, Retained });
coded!(ReadOrder {
    Ascending,
    Descending
});
coded!(ObjectVersions { Revisions, Latest });
product!(SourceTimeRange { start: Time => 1, end: Time => 2 });
product!(ReadLimits { rows: Count => 1, bytes: Count => 2 });
product!(ReadSelection {
    kind: Code<ReadKind> => 1,
    schema: Optional<Nested<SchemaKey>> => 2,
    scope: Optional<Id> => 3,
    stream: Optional<Id> => 4,
    item: Optional<Id> => 5,
    object_key: Optional<Key> => 6,
    object_versions: Code<ObjectVersions> => 7,
    after: Unsigned => 8,
    through: Optional<Unsigned> => 9,
    order: Code<ReadOrder> => 10,
    history: Code<ReadHistory> => 11,
    source_time: Optional<Nested<SourceTimeRange>> => 12,
});

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
