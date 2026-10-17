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
use crate::model::storage::codec::{Nested, product};
use crate::model::storage::{SchemaKey, StorageCursor};
use crate::schema::graph::Schema;
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */
product!(SchemaInput { key: Nested<SchemaKey> => 1, graph: Nested<Schema> => 2 });
product!(SchemaInfo { position: Nested<StorageCursor> => 1, schema: Nested<SchemaInput> => 2 });

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaInput {
    pub key: SchemaKey,
    graph: Schema,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaInfo {
    pub position: StorageCursor,
    pub schema: SchemaInput,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl SchemaInput {
    pub fn new(key: SchemaKey, graph: &Schema) -> Result<Self> {
        graph
            .prepare(&crate::config::SchemaSettings::default())
            .map_err(|_| Error::InvalidInput { field: "storage schema", reason: "invalid finite schema graph" })?;
        Ok(Self { key, graph: graph.clone() })
    }

    pub fn graph(&self) -> &Schema {
        &self.graph
    }
}
impl MemorySize for SchemaInput {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.key.memory_size() - std::mem::size_of::<SchemaKey>()
            + self.graph.memory_size().saturating_sub(std::mem::size_of::<Schema>())
    }
}
impl MemorySize for SchemaInfo {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.position.memory_size() - std::mem::size_of::<StorageCursor>() + self.schema.memory_size()
            - std::mem::size_of::<SchemaInput>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
