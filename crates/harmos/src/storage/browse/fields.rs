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
use std::cmp::Ordering;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::{MAX_SELECTOR_BYTES, heap, invalid};
use crate::storage::ReadOrder;
use crate::storage::object_fields::{
    ObjectComparison, ObjectNullOrder, ObjectScalar, ObjectScalarKind,
};

use crate::{Identifier, MemorySize, Result, schema};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectColumn {
    pub name: Identifier,
    pub tag: u32,
    pub kind: ObjectScalarKind,
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectPredicate {
    pub column: usize,
    pub comparison: ObjectComparison,
    pub value: ObjectScalar,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectOrder {
    pub column: usize,
    pub direction: ReadOrder,
    pub nulls: ObjectNullOrder,
}

/* === Enums === */
/* === Traits === */

/* === Implementations === */
impl MemorySize for ObjectColumn {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(heap(&self.name))
    }
}

impl MemorySize for ObjectPredicate {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(heap(&self.value))
    }
}

impl MemorySize for ObjectOrder {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/* === Public Functions === */
pub(super) fn descriptor(field: &schema::Field) -> Result<ObjectColumn> {
    let optional = match field.cardinality {
        schema::Cardinality::Required => false,
        schema::Cardinality::Optional => true,
        _ => return Err(invalid("field is not a scalar")),
    };
    let kind = match field.kind {
        schema::Kind::Bool => ObjectScalarKind::Bool,
        schema::Kind::U32 => ObjectScalarKind::U32,
        schema::Kind::U64 => ObjectScalarKind::U64,
        schema::Kind::I32 => ObjectScalarKind::I32,
        schema::Kind::I64 => ObjectScalarKind::I64,
        schema::Kind::F32 => ObjectScalarKind::F32,
        schema::Kind::F64 => ObjectScalarKind::F64,
        schema::Kind::String => ObjectScalarKind::String,
        _ => return Err(invalid("field is not a scalar")),
    };
    if field.name.len() > 256 {
        return Err(invalid("field name exceeds query bounds"));
    }
    Ok(ObjectColumn {
        name: Identifier::new(field.name.as_ref())
            .map_err(|_| invalid("invalid scalar field name"))?,
        tag: field.tag,
        kind,
        optional,
    })
}

pub(super) fn check_value(column: &ObjectColumn, value: &ObjectScalar) -> Result<()> {
    let valid = match (column.kind, value) {
        (_, ObjectScalar::Null) => column.optional,
        (ObjectScalarKind::Bool, ObjectScalar::Bool(_)) => true,
        (ObjectScalarKind::U32, ObjectScalar::Unsigned(value)) => u32::try_from(*value).is_ok(),
        (ObjectScalarKind::U64, ObjectScalar::Unsigned(_)) => true,
        (ObjectScalarKind::I32, ObjectScalar::Signed(value)) => i32::try_from(*value).is_ok(),
        (ObjectScalarKind::I64, ObjectScalar::Signed(_)) => true,
        (ObjectScalarKind::F32, ObjectScalar::Float(value)) => {
            value.is_finite() && (*value as f32) as f64 == *value
        }
        (ObjectScalarKind::F64, ObjectScalar::Float(value)) => value.is_finite(),
        (ObjectScalarKind::String, ObjectScalar::Text(_)) => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid("scalar value does not match declared field"))
    }
}

pub(super) fn check_predicate(
    column: &ObjectColumn,
    comparison: ObjectComparison,
    value: &ObjectScalar,
) -> Result<()> {
    check_value(column, value)?;
    if matches!(value, ObjectScalar::Text(value) if value.len() > MAX_SELECTOR_BYTES) {
        return Err(invalid("predicate text exceeds query bounds"));
    }
    if *value == ObjectScalar::Null
        && !matches!(
            comparison,
            ObjectComparison::Equal | ObjectComparison::NotEqual
        )
    {
        return Err(invalid("null requires equality or presence comparison"));
    }
    Ok(())
}

pub(super) fn compare_scalar(left: &ObjectScalar, right: &ObjectScalar) -> Result<Ordering> {
    match (left, right) {
        (ObjectScalar::Bool(a), ObjectScalar::Bool(b)) => Ok(a.cmp(b)),
        (ObjectScalar::Signed(a), ObjectScalar::Signed(b)) => Ok(a.cmp(b)),
        (ObjectScalar::Unsigned(a), ObjectScalar::Unsigned(b)) => Ok(a.cmp(b)),
        (ObjectScalar::Float(a), ObjectScalar::Float(b)) if a.is_finite() && b.is_finite() => {
            a.partial_cmp(b).ok_or(invalid("unordered float"))
        }
        (ObjectScalar::Text(a), ObjectScalar::Text(b)) => Ok(a.cmp(b)),
        _ => Err(invalid("incompatible scalar comparison")),
    }
}

pub(super) fn matches_predicate(
    left: &ObjectScalar,
    comparison: ObjectComparison,
    right: &ObjectScalar,
) -> Result<bool> {
    if *right == ObjectScalar::Null {
        return Ok(match comparison {
            ObjectComparison::Equal => *left == ObjectScalar::Null,
            ObjectComparison::NotEqual => *left != ObjectScalar::Null,
            _ => false,
        });
    }
    if *left == ObjectScalar::Null {
        return Ok(false);
    }
    let ordering = compare_scalar(left, right)?;
    Ok(match comparison {
        ObjectComparison::Equal => ordering == Ordering::Equal,
        ObjectComparison::NotEqual => ordering != Ordering::Equal,
        ObjectComparison::Less => ordering == Ordering::Less,
        ObjectComparison::LessOrEqual => ordering != Ordering::Greater,
        ObjectComparison::Greater => ordering == Ordering::Greater,
        ObjectComparison::GreaterOrEqual => ordering != Ordering::Less,
    })
}

/* === Private Functions === */

/* === Tests === */
