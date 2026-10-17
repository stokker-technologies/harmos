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

// Third Party Imports

// Local Imports
use super::FieldPath;
use crate::MemorySize;
use crate::model::storage::codec::{Code, Flag, Nested, Repeated, coded, product, variants};
use crate::model::storage::query::scalar::ScalarValue;

/* === Constants === */

/* === Macros === */
product!(Filter { clauses: Repeated<Clause> => 1 });
product!(Clause { terms: Repeated<Term> => 1 });
product!(Term { field: Nested<FieldPath> => 1, condition: Nested<Condition> => 2, negate: Flag => 3 });
coded!(Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge
});
variants!(Condition {
    0, 1, "compare": Self::Compare { op, value } => { op: Code<Compare> => 1, value: Nested<ScalarValue> => 2 } => Self::Compare { op, value };
    1, 2, "in": Self::In { values } => { values: Repeated<ScalarValue> => 1 } => Self::In { values };
    2, 3, "is_null": Self::IsNull => {} => Self::IsNull;
});

/* === Type Aliases === */

/* === Structs === */
/// Disjunction of conjunctions. Negation belongs to each typed term.
#[derive(Clone, Debug)]
pub struct Filter {
    pub clauses: Vec<Clause>,
}
#[derive(Clone, Debug)]
pub struct Clause {
    pub terms: Vec<Term>,
}
#[derive(Clone, Debug)]
pub struct Term {
    pub field: FieldPath,
    pub condition: Condition,
    pub negate: bool,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
#[derive(Clone, Debug)]
pub enum Condition {
    Compare { op: Compare, value: ScalarValue },
    In { values: Vec<ScalarValue> },
    IsNull,
}

/* === Traits === */

/* === Implementations === */
impl MemorySize for Filter {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .clauses
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.clauses))
    }
}
impl MemorySize for Clause {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .terms
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.terms))
    }
}
impl MemorySize for Term {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .field
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.field))
            + self
                .condition
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.condition))
    }
}
impl MemorySize for Condition {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Compare { value, .. } => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::In { values } => values
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(values)),
                Self::IsNull => 0,
            }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
