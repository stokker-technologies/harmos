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

// Third Party Imports

// Local Imports
use crate::model::storage::{ObjectKey, SchemaKey, StorageCursor};
use crate::{Identifier, MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RowSlice {
    pub offset: u64,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObjectSelection {
    pub schema: Option<SchemaKey>,
    pub key: Option<ObjectKey>,
    pub versions: ObjectVersions,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecordSelection {
    pub schema: SchemaKey,
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
    pub source_start: Option<SystemTime>,
    pub source_end: Option<SystemTime>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StreamSelection {
    pub schema: SchemaKey,
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
    pub stream: Option<Identifier>,
    pub source_start: Option<SystemTime>,
    pub source_end: Option<SystemTime>,

    pub after: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SchemaSelection {
    pub schema: Option<SchemaKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileSelection {
    pub scope: Option<Identifier>,
    pub item: Option<Identifier>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QueryCompletion {
    pub at: StorageCursor,
    pub rows: u64,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ObjectVersions {
    Latest,
    Revisions,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Source {
    Objects(ObjectSelection),
    Records(RecordSelection),
    Stream(StreamSelection),
    Schemas(SchemaSelection),
    Files(FileSelection),
}

/* === Traits === */

/* === Implementations === */

impl ObjectSelection {
    pub fn follows_revisions(&self) -> bool {
        self.versions == ObjectVersions::Revisions
    }
}

impl Source {
    pub fn grant_scope(&self) -> String {
        match self {
            Self::Objects(selection) => {
                selection.schema.as_ref().map_or_else(|| "objects:*".to_owned(), |schema| format!("objects:schema:{}", schema.id))
            }
            Self::Records(selection) => format!("records:schema:{}", selection.schema.id),
            Self::Stream(selection) => selection
                .stream
                .as_ref()
                .map_or_else(|| format!("streams:schema:{}", selection.schema.id), |stream| format!("streams:id:{stream}")),
            Self::Schemas(selection) => {
                selection.schema.as_ref().map_or_else(|| "schemas:*".to_owned(), |schema| format!("schemas:id:{}", schema.id))
            }
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
                Self::Objects(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Records(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Stream(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Schemas(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Files(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
            }
    }
}
/* === Public Functions === */

/* === Private Functions === */

fn window(start: Option<SystemTime>, end: Option<SystemTime>) -> Result<()> {
    match (start, end) {
        (None, None) => Ok(()),
        (Some(start), Some(end)) if start < end => Ok(()),
        _ => Err(crate::model::storage::query::transform::invalid("source window requires increasing start and end")),
    }
}

/* === Tests === */
