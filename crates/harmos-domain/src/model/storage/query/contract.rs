/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::time::SystemTime;
// Local Imports
use super::transform::Transform;
use crate::model::storage::ArrowIpc;
use crate::model::storage::codec::{
    Code, Id, Nested, Optional, Repeated, Time, Unsigned, coded, product, variants,
};
use crate::model::storage::objects::Key;
use crate::model::storage::{ObjectKey, SchemaKey, StorageCursor};
use crate::{Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct Query {
    pub at: StorageCursor,
    pub source: Source,
    pub transforms: Vec<Transform>,
    pub slice: RowSlice,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowSlice {
    pub offset: u64,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectSelection {
    pub schema: Option<SchemaKey>,
    pub key: Option<ObjectKey>,
    pub versions: ObjectVersions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordSelection {
    pub schema: SchemaKey,
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
    pub source_start: Option<SystemTime>,
    pub source_end: Option<SystemTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamSelection {
    pub schema: SchemaKey,
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
    pub stream: Option<Identifier>,
    pub source_start: Option<SystemTime>,
    pub source_end: Option<SystemTime>,
    /// Only rows stored after this storage position, as `RecordRead::after`.
    pub after: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaSelection {
    pub schema: Option<SchemaKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSelection {
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryCompletion {
    pub at: StorageCursor,
    pub rows: u64,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectVersions {
    Latest,
    Revisions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Objects(ObjectSelection),
    Records(RecordSelection),
    Stream(StreamSelection),
    Schemas(SchemaSelection),
    Files(FileSelection),
}

#[derive(Clone, Debug)]
pub enum QueryOutput {
    Schema(ArrowIpc),
    Batch(ArrowIpc),
}

/* === Traits === */

/* === Implementations === */
impl Query {
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        if self.transforms.len() > 64 {
            return Err(crate::model::storage::query::transform::invalid(
                "too many query transforms",
            ));
        }
        Ok(())
    }
}

impl ObjectSelection {
    pub fn follows_revisions(&self) -> bool {
        self.versions == ObjectVersions::Revisions
    }
}

impl Source {
    /// Exact authorization resource for this selection. Prefixes separate source
    /// kinds and stream identities from schema-wide selectors; `*` names an
    /// unconstrained selection and does not introduce wildcard grant matching.
    pub fn grant_scope(&self) -> String {
        match self {
            Self::Objects(selection) => selection.schema.as_ref().map_or_else(
                || "objects:*".to_owned(),
                |schema| format!("objects:schema:{}", schema.id),
            ),
            Self::Records(selection) => format!("records:schema:{}", selection.schema.id),
            Self::Stream(selection) => selection.stream.as_ref().map_or_else(
                || format!("streams:schema:{}", selection.schema.id),
                |stream| format!("streams:id:{stream}"),
            ),
            Self::Schemas(selection) => selection.schema.as_ref().map_or_else(
                || "schemas:*".to_owned(),
                |schema| format!("schemas:id:{}", schema.id),
            ),
            Self::Files(selection) => match (&selection.scope, &selection.item) {
                (Some(scope), _) => format!("files:scope:{scope}"),
                (None, Some(item)) => format!("files:id:{item}"),
                (None, None) => "files:*".to_owned(),
            },
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Records(selection) => window(selection.source_start, selection.source_end),
            Self::Stream(selection) => window(selection.source_start, selection.source_end),
            Self::Objects(_) | Self::Schemas(_) | Self::Files(_) => Ok(()),
        }
    }
}

product!(Query {
    at: Nested<StorageCursor> => 1,
    source: Nested<Source> => 2,
    transforms: Repeated<Transform> => 3,
    slice: Nested<RowSlice> => 4,
});
product!(RowSlice { offset: Unsigned => 1, limit: Optional<Unsigned> => 2 });
product!(ObjectSelection {
    schema: Optional<Nested<SchemaKey>> => 1,
    key: Optional<Key> => 2,
    versions: Code<ObjectVersions> => 3,
});
product!(RecordSelection {
    schema: Nested<SchemaKey> => 1,
    scope: Optional<Id> => 2,
    item: Optional<Id> => 3,
    source_start: Optional<Time> => 4,
    source_end: Optional<Time> => 5,
});
product!(StreamSelection {
    schema: Nested<SchemaKey> => 1,
    scope: Optional<Id> => 2,
    item: Optional<Id> => 3,
    stream: Optional<Id> => 4,
    source_start: Optional<Time> => 5,
    source_end: Optional<Time> => 6,
    after: Unsigned => 7,
});
product!(SchemaSelection { schema: Optional<Nested<SchemaKey>> => 1 });
product!(FileSelection { scope: Optional<Id> => 1, item: Optional<Id> => 2 });
product!(QueryCompletion { at: Nested<StorageCursor> => 1, rows: Unsigned => 2 });
coded!(ObjectVersions { Latest, Revisions });
variants!(Source {
    0, 1, "objects": Self::Objects(value) => { value: Nested<ObjectSelection> => 1 } => Self::Objects(value);
    1, 2, "records": Self::Records(value) => { value: Nested<RecordSelection> => 1 } => Self::Records(value);
    2, 3, "stream": Self::Stream(value) => { value: Nested<StreamSelection> => 1 } => Self::Stream(value);
    3, 4, "schemas": Self::Schemas(value) => { value: Nested<SchemaSelection> => 1 } => Self::Schemas(value);
    4, 5, "files": Self::Files(value) => { value: Nested<FileSelection> => 1 } => Self::Files(value);
});
variants!(QueryOutput {
    0, 1, "schema": Self::Schema(value) => { value: Nested<ArrowIpc> => 1 } => Self::Schema(value);
    1, 2, "batch": Self::Batch(value) => { value: Nested<ArrowIpc> => 1 } => Self::Batch(value);
});

macro_rules! memory {
    ($ty:ty, $($field:ident),+ $(,)?) => {
        impl MemorySize for $ty {
            fn memory_size(&self) -> usize {
                std::mem::size_of::<Self>()$(.saturating_add(self.$field.memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.$field))))+
            }
        }
    };
}
memory!(Query, at, source, transforms, slice);
memory!(RowSlice, offset, limit);
memory!(ObjectSelection, schema, key);
memory!(RecordSelection, schema, scope, item);
memory!(StreamSelection, schema, scope, item, stream);
memory!(SchemaSelection, schema);
memory!(FileSelection, scope, item);
memory!(QueryCompletion, at);
impl MemorySize for Source {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Objects(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Records(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Stream(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Schemas(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Files(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
            }
    }
}
impl MemorySize for QueryOutput {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Schema(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Batch(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
            }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn window(start: Option<SystemTime>, end: Option<SystemTime>) -> Result<()> {
    match (start, end) {
        (None, None) => Ok(()),
        (Some(start), Some(end)) if start < end => Ok(()),
        _ => Err(crate::model::storage::query::transform::invalid(
            "source window requires increasing start and end",
        )),
    }
}

/* === Tests === */
