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

pub use harmos_domain::model::storage::query::transform::sort::{Sort, SortKey};

/* === Imports === */

use crate::storage::query::transform::FieldPathExecution;
// Standard Library Imports
use std::cmp::Ordering;
// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::Schema;
// Local Imports
use super::{invalid, scalar_type, take};
use crate::Result;
use crate::storage::query::accumulate::sort_values;
use harmos_domain::model::storage::query::scalar::ScalarValue;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait SortExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch>;
}

/* === Implementations === */

impl SortExecution for Sort {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.keys.is_empty() || self.keys.len() > 64 {
            return Err(invalid("sort requires 1..64 keys"));
        }
        for key in &self.keys {
            if !scalar_type(key.field.field(input)?.data_type()) {
                return Err(invalid("sort requires scalar fields"));
            }
        }
        Ok(input.clone())
    }

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        let mut indices = (0..input.num_rows() as u64).collect::<Vec<_>>();
        sort_values(&mut indices, |left, right| {
            check()?;
            for key in &self.keys {
                let left = key.field.value(input, left as usize)?;
                let right = key.field.value(input, right as usize)?;
                let absent = |value: &ScalarValue| {
                    matches!(value, ScalarValue::Null | ScalarValue::Missing)
                        || matches!(value, ScalarValue::Float(value) if !value.is_finite())
                };
                let null_order = match (absent(&left), absent(&right)) {
                    (true, true) => Ordering::Equal,
                    (true, false) => {
                        if key.nulls_first {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        }
                    }
                    (false, true) => {
                        if key.nulls_first {
                            Ordering::Greater
                        } else {
                            Ordering::Less
                        }
                    }
                    (false, false) => {
                        let order = left.compare(&right)?;
                        if key.descending { order.reverse() } else { order }
                    }
                };
                if !null_order.is_eq() {
                    return Ok(null_order);
                }
            }
            // Original row order is a final tie key, independent of sort direction.
            Ok(left.cmp(&right))
        })?;
        take(input, indices)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use crate::query::transform::{
        FieldPath, Transform, TransformExecution,
        project::{Project, Projection},
        sort::{Sort, SortKey},
    };
    use arrow_array::{Array, Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    fn batch() -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![Field::new("key", DataType::Int64, false), Field::new("row", DataType::Int64, false)]));
        RecordBatch::try_new(schema, vec![Arc::new(Int64Array::from(vec![2, 1, 1, 2])), Arc::new(Int64Array::from(vec![0, 1, 2, 3]))])
            .unwrap()
    }

    #[test]
    fn sort_keeps_ties() {
        // Sort Keeps Stable Ties Before Projection Selects The Original Row Order.
        let original = batch();
        let sorted = Transform::Sort(Sort {
            keys: vec![SortKey {
                field: FieldPath::from("key"),
                descending: false,
                nulls_first: false,
            }],
        })
        .apply(&original, &mut || Ok(()))
        .unwrap();
        let projected = Transform::Project(Project {
            fields: vec![Projection {
                field: FieldPath::from("row"),
                name: "selected".into(),
            }],
        })
        .apply(&sorted, &mut || Ok(()))
        .unwrap();
        let values = projected.column(0).as_any().downcast_ref::<Int64Array>().unwrap();
        assert_eq!(values.values().as_ref(), &[1, 2, 0, 3]);
    }
}
