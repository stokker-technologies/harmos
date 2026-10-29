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

pub use harmos_domain::model::storage::query::transform::filter::{Clause, Compare, Condition, Filter, Term};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::Schema;
use harmos_domain::model::storage::query::scalar::ScalarValue;

// Local Imports
use super::{invalid, scalar_type, take};
use crate::Result;
use crate::storage::query::transform::FieldPathExecution;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait FilterExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch>;

    fn select(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<Vec<u64>>;
}

/* === Implementations === */

impl FilterExecution for Filter {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.clauses.is_empty()
            || self.clauses.len() > 64
            || self.clauses.iter().any(|clause| clause.terms.is_empty())
            || self.clauses.iter().map(|clause| clause.terms.len()).sum::<usize>() > 256
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
                    (arrow_schema::DataType::Float32 | arrow_schema::DataType::Float64, ScalarValue::Float(_)) => true,
                    (arrow_schema::DataType::Utf8, ScalarValue::Text(_)) => true,
                    (arrow_schema::DataType::Timestamp(_, _), ScalarValue::Datetime(_)) => true,
                    (kind, ScalarValue::Exact(_) | ScalarValue::Signed(_) | ScalarValue::Unsigned(_)) if kind == &super::exact_type() => {
                        true
                    }
                    _ => false,
                };
                if !compatible {
                    return Err(invalid("filter literal type differs from field"));
                }
            }
        }
        Ok(input.clone())
    }

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        take(input, self.select(input, check)?)
    }

    fn select(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<Vec<u64>> {
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
                        Condition::Compare { op, value: expected } => compare(&value, *op, expected)?,
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
