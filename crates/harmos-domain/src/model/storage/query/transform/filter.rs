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
use super::FieldPath;
use crate::MemorySize;
use crate::model::storage::query::scalar::ScalarValue;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Filter {
    pub clauses: Vec<Clause>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Clause {
    pub terms: Vec<Term>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Term {
    pub field: FieldPath,
    pub condition: Condition,
    pub negate: bool,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Condition {
    Compare { op: Compare, value: ScalarValue },
    In { values: Vec<ScalarValue> },
    IsNull,
}

/* === Traits === */

/* === Implementations === */

impl MemorySize for Filter {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.clauses.memory_size().saturating_sub(std::mem::size_of_val(&self.clauses))
    }
}
impl MemorySize for Clause {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.terms.memory_size().saturating_sub(std::mem::size_of_val(&self.terms))
    }
}
impl MemorySize for Term {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.field.memory_size().saturating_sub(std::mem::size_of_val(&self.field))
            + self.condition.memory_size().saturating_sub(std::mem::size_of_val(&self.condition))
    }
}
impl MemorySize for Condition {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Compare { value, .. } => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::In { values } => values.memory_size().saturating_sub(std::mem::size_of_val(values)),
                Self::IsNull => 0,
            }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
