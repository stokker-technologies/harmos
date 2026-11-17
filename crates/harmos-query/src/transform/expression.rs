/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Streaming Field Expressions

/* === Modules === */

/* === Re-Exports === */

pub use harmos_domain::model::storage::query::transform::expression::{Contains, Offset, Scale};

/* === Imports === */

// Standard Library Imports
use std::cmp::Ordering;
use std::sync::Arc;

// Third Party Imports
use arrow_array::{Array, ArrayRef, BooleanArray, FixedSizeListArray, Float64Array, LargeListArray, ListArray, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};

// Local Imports
use super::{FieldPath, FieldPathExecution, array_at, invalid, literal_matches, numeric_type, scalar, scalar_type};
use crate::Result;
use harmos_domain::model::storage::query::scalar::ScalarValue;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn numeric_schema(input: &Schema, field: &FieldPath, name: &str, operand: f64) -> Result<Schema> {
    if !operand.is_finite() {
        return Err(invalid("supply a finite scale or offset"));
    }
    let field = field.field(input)?;
    if !numeric_type(field.data_type()) {
        return Err(invalid("select a numeric field for scale or offset"));
    }
    output_schema(input, name, DataType::Float64, true)
}

pub(super) fn numeric(
    input: &RecordBatch, field: &FieldPath, name: &str, operand: f64, evaluate: impl Fn(f64) -> f64, check: &mut impl FnMut() -> Result<()>,
) -> Result<RecordBatch> {
    let schema = numeric_schema(input.schema().as_ref(), field, name, operand)?;
    let mut values = Vec::with_capacity(input.num_rows());
    for row in 0..input.num_rows() {
        check()?;
        let number = match field.value(input, row)? {
            ScalarValue::Signed(value) => Some(value as f64),
            ScalarValue::Unsigned(value) => Some(value as f64),
            ScalarValue::Float(value) if value.is_finite() => Some(value),
            ScalarValue::Exact(value) => {
                let magnitude = value.numerator as f64 / value.denominator.get() as f64;
                Some(if value.negative { -magnitude } else { magnitude })
            }
            ScalarValue::Null | ScalarValue::Missing | ScalarValue::Float(_) => None,
            _ => return Err(invalid("select a numeric field for scale or offset")),
        };
        let value = number.map(&evaluate);
        if value.is_some_and(|value| !value.is_finite()) {
            return Err(invalid("expression overflow; reduce the scale or offset"));
        }
        values.push(value);
    }
    let values = Arc::new(Float64Array::from(values));
    output(input, schema, name, values)
}

pub(super) fn contains_schema(input: &Schema, plan: &Contains) -> Result<Schema> {
    plan.value.check()?;
    let field = plan.field.field(input)?;
    let compatible = match field.data_type() {
        DataType::Utf8 => matches!(plan.value, ScalarValue::Text(_)),
        DataType::List(child) | DataType::LargeList(child) | DataType::FixedSizeList(child, _) => {
            scalar_type(child.data_type()) && literal_matches(child.data_type(), &plan.value)
        }
        _ => false,
    };
    if !compatible {
        return Err(invalid("select text with a text needle, or a scalar list with a matching nonnull needle"));
    }
    output_schema(input, &plan.name, DataType::Boolean, field.is_nullable())
}

pub(super) fn contains(input: &RecordBatch, plan: &Contains, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
    let schema = contains_schema(input.schema().as_ref(), plan)?;
    let mut values = Vec::with_capacity(input.num_rows());
    for row in 0..input.num_rows() {
        check()?;
        let Some(array) = array_at(&plan.field, input, row)? else {
            values.push(None);
            continue;
        };
        if let Some(text) = array.as_any().downcast_ref::<StringArray>() {
            let ScalarValue::Text(needle) = &plan.value else { return Err(invalid("supply a text needle")) };
            values.push(Some(text.value(row).contains(needle.as_str())));
            continue;
        }
        let children = if let Some(list) = array.as_any().downcast_ref::<ListArray>() {
            list.value(row)
        } else if let Some(list) = array.as_any().downcast_ref::<LargeListArray>() {
            list.value(row)
        } else if let Some(list) = array.as_any().downcast_ref::<FixedSizeListArray>() {
            list.value(row)
        } else {
            return Err(invalid("select a supported scalar list"));
        };
        let mut found = false;
        for child in 0..children.len() {
            check()?;
            if children.is_null(child) {
                continue;
            }
            let value = scalar(children.as_ref(), child)?;
            if matches!(value, ScalarValue::Float(number) if !number.is_finite()) {
                continue;
            }
            if value.compare(&plan.value)? == Ordering::Equal {
                found = true;
                break;
            }
        }
        values.push(Some(found));
    }
    let values = Arc::new(BooleanArray::from(values));
    output(input, schema, &plan.name, values)
}

/* === Private Functions === */

fn output_schema(input: &Schema, name: &str, data_type: DataType, nullable: bool) -> Result<Schema> {
    let ambiguous = input.fields().iter().filter(|field| field.name() == name).count() > 1;
    if name.is_empty() || ambiguous {
        return Err(invalid("supply a nonempty unambiguous expression output name"));
    }
    let field = Field::new(name, data_type, nullable);
    let mut fields = input.fields().iter().map(|field| field.as_ref().clone()).collect::<Vec<_>>();
    match fields.iter().position(|field| field.name() == name) {
        Some(index) => fields[index] = field,
        None => fields.push(field),
    }
    Ok(Schema::new_with_metadata(fields, input.metadata().clone()))
}

fn output(input: &RecordBatch, schema: Schema, name: &str, values: ArrayRef) -> Result<RecordBatch> {
    let mut columns = input.columns().to_vec();
    match input.schema().index_of(name).ok() {
        Some(index) => columns[index] = values,
        None => columns.push(values),
    }
    RecordBatch::try_new(Arc::new(schema), columns).map_err(|_| invalid("inspect expression inputs and output schema"))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::super::{Transform, TransformExecution};
    use super::*;
    use crate::Error;
    use arrow_array::{StructArray, types::Int64Type};
    use arrow_buffer::NullBuffer;

    fn batch() -> RecordBatch {
        let lists = ListArray::from_iter_primitive::<Int64Type, _, _>(vec![Some(vec![None, Some(1)]), Some(vec![Some(2)]), Some(vec![])]);
        let fields = vec![
            Field::new("number", DataType::Float64, true),
            Field::new("text", DataType::Utf8, true),
            Field::new("list", lists.data_type().clone(), false),
        ];
        let payload = StructArray::new(
            fields.into(),
            vec![
                Arc::new(Float64Array::from(vec![Some(2.0), Some(9.0), None])),
                Arc::new(StringArray::from(vec![Some("αbeta"), Some("ghost"), None])),
                Arc::new(lists),
            ],
            Some(NullBuffer::from(vec![true, false, true])),
        );
        RecordBatch::try_from_iter(vec![("payload", Arc::new(payload) as ArrayRef)]).unwrap()
    }

    #[test]
    fn arithmetic_preserves_parent_and_field_nulls() {
        let input = batch();
        let scale = Transform::Scale(Scale {
            field: "payload.number".into(),
            name: "scaled".into(),
            factor: 2.0,
        });
        let scaled = scale.apply(&input, &mut || Ok(())).unwrap();
        let offset = Transform::Offset(Offset {
            field: "scaled".into(),
            name: "shifted".into(),
            amount: 1.0,
        });
        let shifted = offset.apply(&scaled, &mut || Ok(())).unwrap();
        let values = shifted.column_by_name("shifted").unwrap().as_any().downcast_ref::<Float64Array>().unwrap();
        assert_eq!(values.iter().collect::<Vec<_>>(), [Some(5.0), None, None]);
    }

    #[test]
    fn containment_preserves_parent_nulls_and_ignores_absent_members() {
        let input = batch();
        let text = Transform::Contains(Contains {
            field: "payload.text".into(),
            name: "found".into(),
            value: ScalarValue::Text("beta".into()),
        });
        let result = text.apply(&input, &mut || Ok(())).unwrap();
        let values = result.column_by_name("found").unwrap().as_any().downcast_ref::<BooleanArray>().unwrap();
        assert_eq!(values.iter().collect::<Vec<_>>(), [Some(true), None, None]);
        let list = Transform::Contains(Contains {
            field: "payload.list".into(),
            name: "found".into(),
            value: ScalarValue::Signed(1),
        });
        let result = list.apply(&input, &mut || Ok(())).unwrap();
        let values = result.column_by_name("found").unwrap().as_any().downcast_ref::<BooleanArray>().unwrap();
        assert_eq!(values.iter().collect::<Vec<_>>(), [Some(true), None, Some(false)]);
    }

    #[test]
    fn expressions_refuse_invalid_operands_and_overflow() {
        let input = batch();
        let invalid_scale = Transform::Scale(Scale {
            field: "payload.number".into(),
            name: "x".into(),
            factor: f64::INFINITY,
        });
        assert!(invalid_scale.apply(&input, &mut || Ok(())).is_err());
        let overflow = Transform::Scale(Scale {
            field: "payload.number".into(),
            name: "x".into(),
            factor: f64::MAX,
        });
        assert!(overflow.apply(&input, &mut || Ok(())).is_err());
        let wrong_type = Transform::Contains(Contains {
            field: "payload.list".into(),
            name: "x".into(),
            value: ScalarValue::Text("1".into()),
        });
        assert!(wrong_type.apply(&input, &mut || Ok(())).is_err());
    }

    #[test]
    fn expressions_observe_cancellation_during_a_batch() {
        let input = batch();
        let scale = Transform::Scale(Scale {
            field: "payload.number".into(),
            name: "x".into(),
            factor: 2.0,
        });
        let mut calls = 0;
        let result = scale.apply(&input, &mut || {
            calls += 1;
            if calls == 2 {
                return Err(Error::Closing { domain: "cancelled query" });
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Closing { .. })));
    }
}
