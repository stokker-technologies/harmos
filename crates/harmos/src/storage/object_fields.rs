/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod codec;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports
use crate::storage::{ObjectKey, ObjectVersionToken, SchemaKey, StorageCursor};
use crate::{Error, Identifier, MemorySize, Result};

/* === Constants === */
pub(crate) const MAX_COLUMNS: usize = 64;
pub(crate) const MAX_SELECTOR_BYTES: usize = 65_536;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectFields {
    pub projection: Vec<Identifier>,
    pub filters: Vec<ObjectFilter>,
    pub order: Vec<ObjectSort>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectFilter {
    pub field: Identifier,
    pub comparison: ObjectComparison,
    pub value: ObjectScalar,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectSort {
    pub field: Identifier,
    pub descending: bool,
    pub nulls: ObjectNullOrder,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectMetadata {
    pub schema: SchemaKey,
    pub item: ObjectKey,
    pub position: StorageCursor,
    pub version: ObjectVersionToken,
    pub values: Vec<ObjectScalar>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectScalarKind {
    Bool,
    U32,
    U64,
    I32,
    I64,
    F32,
    F64,
    String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ObjectScalar {
    Null,
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
    Float(f64),
    Text(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectComparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectNullOrder {
    First,
    Last,
}

/* === Traits === */

/* === Implementations === */
impl ObjectFields {
    pub fn validate(&self) -> Result<()> {
        if self.projection.len() > MAX_COLUMNS
            || self.filters.len() > MAX_COLUMNS
            || self.order.len() > MAX_COLUMNS
        {
            return Err(invalid(
                "too many projection fields, filters or sort fields",
            ));
        }
        for name in self
            .projection
            .iter()
            .chain(self.filters.iter().map(|filter| &filter.field))
            .chain(self.order.iter().map(|order| &order.field))
        {
            if name.as_str().len() > 256 {
                return Err(invalid("field name exceeds query bounds"));
            }
        }
        for filter in &self.filters {
            if matches!(&filter.value, ObjectScalar::Text(text) if text.len() > MAX_SELECTOR_BYTES)
            {
                return Err(invalid("predicate text exceeds query bounds"));
            }
        }
        Ok(())
    }
}
impl MemorySize for ObjectFields {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + heap(&self.projection)
            + heap(&self.filters)
            + heap(&self.order)
    }
}
impl MemorySize for ObjectFilter {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + heap(&self.field) + heap(&self.value)
    }
}
impl MemorySize for ObjectSort {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + heap(&self.field)
    }
}
impl MemorySize for ObjectScalar {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(match self {
            Self::Text(value) => value.capacity(),
            _ => 0,
        })
    }
}

impl MemorySize for ObjectMetadata {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.schema))
            .saturating_add(heap(&self.item))
            .saturating_add(heap(&self.position))
            .saturating_add(heap(&self.version))
            .saturating_add(heap(&self.values))
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn heap<T: MemorySize>(value: &T) -> usize {
    value.memory_size().saturating_sub(std::mem::size_of::<T>())
}
fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "object fields",
        reason,
    }
}

/* === Tests === */
