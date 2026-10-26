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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::model::storage::codec::{Nested, Shared, product};
use crate::model::storage::{SchemaKey, StorageCursor};
use crate::schema::graph::Schema;
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */

product!(SchemaInput { key: Nested<SchemaKey> => 1, graph: Shared<Schema> => 2 });
product!(SchemaInfo { position: Nested<StorageCursor> => 1, schema: Nested<SchemaInput> => 2 });

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaInput {
    pub key: SchemaKey,
    graph: Arc<Schema>,
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
    pub fn new(key: SchemaKey, graph: impl Into<Arc<Schema>>) -> Result<Self> {
        let graph = graph.into();
        graph
            .prepare(&crate::config::SchemaSettings::default())
            .map_err(|_| Error::InvalidInput { field: "storage schema", reason: "invalid finite schema graph" })?;
        Ok(Self { key, graph })
    }

    pub fn graph(&self) -> &Schema {
        &self.graph
    }

    pub fn graph_owner(&self) -> Arc<Schema> {
        Arc::clone(&self.graph)
    }
}
impl MemorySize for SchemaInput {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.key.memory_size() - std::mem::size_of::<SchemaKey>()
            + self.graph.memory_size()
            + 2 * std::mem::size_of::<usize>()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identifier, Message};

    #[test]
    fn graph_owner_retained() {
        let mut definitions = Vec::with_capacity(8);
        definitions.push(Vec::with_capacity(16));
        let pointer = definitions.as_ptr();
        let graph = Schema { root: 0, definitions };
        let key = SchemaKey {
            id: Identifier::new("owned.schema").unwrap(),
            version: std::num::NonZeroU32::MIN,
            payload_encoding: crate::model::storage::PayloadEncoding::PostcardV1,
        };
        let input = SchemaInput::new(key, graph).unwrap();
        assert_eq!(input.graph().definitions.as_ptr(), pointer);
        assert_eq!(input.graph().definitions.capacity(), 8);
        let retained = input.graph_owner();
        let duplicate = input.clone();
        assert!(Arc::ptr_eq(&retained, &duplicate.graph_owner()));
        let encoded = input.encode_to_vec();
        let decoded = SchemaInput::decode(&encoded).unwrap();
        assert_eq!(decoded, input);
        assert!(input.memory_size() >= input.graph().memory_size());
    }
}
