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
// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::Schema;
// Local Imports
use super::{FieldPath, invalid, scalar_type, take};
use crate::storage::codec::{Code, Flag, Nested, Repeated, coded, product, variants};
use crate::storage::query::scalar::ScalarValue;
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

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
impl Filter {
    pub fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.clauses.is_empty()
            || self.clauses.len() > 64
            || self.clauses.iter().any(|clause| clause.terms.is_empty())
            || self
                .clauses
                .iter()
                .map(|clause| clause.terms.len())
                .sum::<usize>()
                > 256
        {
            return Err(invalid("filter requires nonempty bounded clauses"));
        }
        for term in self.clauses.iter().flat_map(|clause| &clause.terms) {
            let field = term.field.field(input)?;
            if !scalar_type(field.data_type()) {
                return Err(invalid("filter requires scalar fields"));
            }
            let values: &[ScalarValue] = match &term.condition {
                Condition::Compare { value, .. } => std::slice::from_ref(value),
                Condition::In { values } if values.len() <= 4096 => values,
                Condition::In { .. } => return Err(invalid("filter set is too large")),
                Condition::IsNull => &[],
            };
            for value in values {
                value.check()?;
                if matches!(value, ScalarValue::Null | ScalarValue::Missing) {
                    return Err(invalid("use IsNull for absent value tests"));
                }
                let compatible = match (field.data_type(), value) {
                    (arrow_schema::DataType::Boolean, ScalarValue::Bool(_)) => true,
                    (
                        arrow_schema::DataType::Int8
                        | arrow_schema::DataType::Int16
                        | arrow_schema::DataType::Int32
                        | arrow_schema::DataType::Int64,
                        ScalarValue::Signed(_),
                    ) => true,
                    (
                        arrow_schema::DataType::UInt8
                        | arrow_schema::DataType::UInt16
                        | arrow_schema::DataType::UInt32
                        | arrow_schema::DataType::UInt64,
                        ScalarValue::Unsigned(_),
                    ) => true,
                    (
                        arrow_schema::DataType::Float32 | arrow_schema::DataType::Float64,
                        ScalarValue::Float(_),
                    ) => true,
                    (arrow_schema::DataType::Utf8, ScalarValue::Text(_)) => true,
                    (arrow_schema::DataType::Timestamp(_, _), ScalarValue::Datetime(_)) => true,
                    (
                        kind,
                        ScalarValue::Exact(_) | ScalarValue::Signed(_) | ScalarValue::Unsigned(_),
                    ) if kind == &super::exact_type() => true,
                    _ => false,
                };
                if !compatible {
                    return Err(invalid("filter literal type differs from field"));
                }
            }
        }
        Ok(input.clone())
    }

    pub fn apply(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<RecordBatch> {
        take(input, self.select(input, check)?)
    }

    /// Ascending indices of the rows of `input` this filter keeps.
    pub(crate) fn select(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<Vec<u64>> {
        let mut selected = Vec::new();
        for row in 0..input.num_rows() {
            check()?;
            let mut matches = false;
            for clause in &self.clauses {
                let mut all = true;
                for term in &clause.terms {
                    let value = term.field.value(input, row)?;
                    let absent = matches!(value, ScalarValue::Null | ScalarValue::Missing)
                        || matches!(value, ScalarValue::Float(number) if !number.is_finite());
                    let answer = match &term.condition {
                        Condition::IsNull => absent,
                        _ if absent => false,
                        Condition::Compare {
                            op,
                            value: expected,
                        } => compare(&value, *op, expected)?,
                        Condition::In { values } => {
                            let mut found = false;
                            for expected in values {
                                if compare(&value, Compare::Eq, expected)? {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        }
                    };
                    // Unknown comparisons remain nonmatching under negation.
                    let answer = if absent && !matches!(&term.condition, Condition::IsNull) {
                        false
                    } else if term.negate {
                        !answer
                    } else {
                        answer
                    };
                    if !answer {
                        all = false;
                        break;
                    }
                }
                if all {
                    matches = true;
                    break;
                }
            }
            if matches {
                selected.push(row as u64);
            }
        }
        Ok(selected)
    }
}
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
fn compare(value: &ScalarValue, op: Compare, expected: &ScalarValue) -> Result<bool> {
    let order = value.compare(expected)?;
    Ok(match op {
        Compare::Eq => order.is_eq(),
        Compare::Ne => !order.is_eq(),
        Compare::Lt => order.is_lt(),
        Compare::Le => !order.is_gt(),
        Compare::Gt => order.is_gt(),
        Compare::Ge => !order.is_lt(),
    })
}

/* === Tests === */
