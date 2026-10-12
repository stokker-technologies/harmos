/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod fields;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::schema::Field;
use crate::storage::codec::{Nested, product};
use crate::storage::{SchemaKey, StorageCursor};
use crate::{Error, MemorySize, Result};

/* === Constants === */

/* === Macros === */
product!(SchemaInput { key: Nested<SchemaKey> => 1, fields: fields::Fields => 2 });
product!(SchemaInfo { position: Nested<StorageCursor> => 1, schema: Nested<SchemaInput> => 2 });

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaInput {
    pub key: SchemaKey,
    fields: Vec<Field>,
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
    pub fn new(key: SchemaKey, fields: &[Field]) -> Result<Self> {
        crate::schema::validate(fields).map_err(|_| Error::InvalidInput {
            field: "storage schema",
            reason: "invalid message fields",
        })?;
        Ok(Self {
            key,
            fields: fields.to_vec(),
        })
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
}
impl MemorySize for SchemaInput {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.key.memory_size() - std::mem::size_of::<SchemaKey>()
            + fields::resident(&self.fields)
    }
}
impl MemorySize for SchemaInfo {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.position.memory_size()
            - std::mem::size_of::<StorageCursor>()
            + self.schema.memory_size()
            - std::mem::size_of::<SchemaInput>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
