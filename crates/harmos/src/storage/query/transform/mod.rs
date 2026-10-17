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
pub mod aggregate;
pub mod downsample;
pub mod filter;
pub mod histogram;
pub mod project;
pub mod sort;

/* === Re-Exports === */
pub use aggregate::AggregateExecution;
pub use downsample::DownsampleExecution;
pub use filter::FilterExecution;
pub use harmos_domain::model::storage::query::transform::{FieldPath, Transform};
pub use histogram::HistogramExecution;
pub use project::ProjectExecution;
pub use sort::SortExecution;

/* === Imports === */
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
use crate::{Error, Result};
use arrow_array::{
    Array, ArrayRef, BinaryArray, BooleanArray, Float32Array, Float64Array, Int8Array, Int16Array,
    Int32Array, Int64Array, NullArray, RecordBatch, StringArray, StructArray,
    TimestampNanosecondArray, UInt8Array, UInt16Array, UInt32Array, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use harmos_domain::model::storage::query::scalar::{ExactNumber, ScalarValue};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
pub trait FieldPathExecution {
    fn field(&self, schema: &Schema) -> Result<Field>;

    fn array<'a>(&self, batch: &'a RecordBatch) -> Result<&'a ArrayRef>;

    fn value(&self, batch: &RecordBatch, row: usize) -> Result<ScalarValue>;
}

pub trait TransformExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn apply(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<RecordBatch>;
}

/* === Implementations === */

impl FieldPathExecution for FieldPath {
    fn field(&self, schema: &Schema) -> Result<Field> {
        if self.fields.is_empty() || self.fields.len() > 32 {
            return Err(invalid("field path requires 1..32 components"));
        }
        let mut fields = schema.fields();
        let mut found = None;
        let mut nullable = false;
        for (index, name) in self.fields.iter().enumerate() {
            let field = fields
                .iter()
                .find(|field| field.name() == name)
                .ok_or_else(|| invalid("query field does not exist"))?;
            nullable |= field.is_nullable();
            found = Some(field.as_ref().clone().with_nullable(nullable));
            if index + 1 < self.fields.len() {
                let DataType::Struct(children) = field.data_type() else {
                    return Err(invalid("field path traverses a non-struct value"));
                };
                fields = children;
            }
        }
        Ok(found.expect("nonempty field path"))
    }

    fn array<'a>(&self, batch: &'a RecordBatch) -> Result<&'a ArrayRef> {
        let first = self
            .fields
            .first()
            .ok_or_else(|| invalid("empty field path"))?;
        let mut array = batch
            .column_by_name(first)
            .ok_or_else(|| invalid("query field does not exist"))?;
        for name in self.fields.iter().skip(1) {
            let structure = array
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or_else(|| invalid("field path traverses a non-struct value"))?;
            array = structure
                .column_by_name(name)
                .ok_or_else(|| invalid("query child field does not exist"))?;
        }
        Ok(array)
    }

    fn value(&self, batch: &RecordBatch, row: usize) -> Result<ScalarValue> {
        let first = self
            .fields
            .first()
            .ok_or_else(|| invalid("empty field path"))?;
        let mut array = batch
            .column_by_name(first)
            .ok_or_else(|| invalid("query field does not exist"))?;
        if array.is_null(row) {
            return Ok(ScalarValue::Null);
        }
        for name in self.fields.iter().skip(1) {
            let structure = array
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or_else(|| invalid("field path traverses a non-struct value"))?;
            array = structure
                .column_by_name(name)
                .ok_or_else(|| invalid("query child field does not exist"))?;
            if array.is_null(row) {
                return Ok(ScalarValue::Null);
            }
        }
        scalar(array.as_ref(), row)
    }
}

impl TransformExecution for Transform {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        match self {
            Self::Filter(value) => value.output_schema(input),
            Self::Project(value) => value.output_schema(input),
            Self::Aggregate(value) => value.output_schema(input),
            Self::Histogram(value) => value.output_schema(input),
            Self::Downsample(value) => value.output_schema(input),
            Self::Sort(value) => value.output_schema(input),
        }
    }

    fn apply(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<RecordBatch> {
        match self {
            Self::Filter(value) => value.apply(input, check),
            Self::Project(value) => value.apply(input, check),
            Self::Aggregate(value) => value.apply(input, check),
            Self::Histogram(value) => value.apply(input, check),
            Self::Downsample(value) => value.apply(input, check),
            Self::Sort(value) => value.apply(input, check),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
pub(crate) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "storage query transform",
        reason,
    }
}

pub(super) fn take(input: &RecordBatch, indices: Vec<u64>) -> Result<RecordBatch> {
    let indices = UInt64Array::from(indices);
    let columns = input
        .columns()
        .iter()
        .map(|column| {
            arrow_select::take::take(column.as_ref(), &indices, None)
                .map_err(|_| invalid("Arrow row selection failed"))
        })
        .collect::<Result<Vec<_>>>()?;
    RecordBatch::try_new_with_options(
        input.schema(),
        columns,
        &arrow_array::RecordBatchOptions::new().with_row_count(Some(indices.len())),
    )
    .map_err(|_| invalid("Arrow selected batch is invalid"))
}

pub(super) fn scalar(array: &dyn Array, row: usize) -> Result<ScalarValue> {
    if array.is_null(row) {
        return Ok(ScalarValue::Null);
    }
    macro_rules! read {
        ($ty:ty, $variant:ident, $convert:expr) => {
            if let Some(array) = array.as_any().downcast_ref::<$ty>() {
                return Ok(ScalarValue::$variant(($convert)(array.value(row))));
            }
        };
    }
    read!(BooleanArray, Bool, |value| value);
    read!(Int8Array, Signed, i64::from);
    read!(Int16Array, Signed, i64::from);
    read!(Int32Array, Signed, i64::from);
    read!(Int64Array, Signed, |value| value);
    read!(UInt8Array, Unsigned, u64::from);
    read!(UInt16Array, Unsigned, u64::from);
    read!(UInt32Array, Unsigned, u64::from);
    read!(UInt64Array, Unsigned, |value| value);
    read!(Float32Array, Float, f64::from);
    read!(Float64Array, Float, |value| value);
    read!(StringArray, Text, str::to_owned);
    read!(TimestampNanosecondArray, Datetime, i128::from);
    if let Some(exact) = array.as_any().downcast_ref::<StructArray>() {
        return exact_value(exact, row);
    }
    Err(invalid("field is not a supported scalar"))
}

pub(super) fn exact_type() -> DataType {
    DataType::Struct(
        vec![
            Arc::new(Field::new("negative", DataType::Boolean, false)),
            Arc::new(Field::new("numerator", DataType::Binary, false)),
            Arc::new(Field::new("denominator", DataType::Binary, false)),
        ]
        .into(),
    )
}

fn exact_value(value: &StructArray, row: usize) -> Result<ScalarValue> {
    if value.data_type() != &exact_type() {
        return Err(invalid("struct is not an exact number"));
    }
    let sign = value
        .column(0)
        .as_any()
        .downcast_ref::<BooleanArray>()
        .expect("validated exact sign");
    let numerator = value
        .column(1)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("validated exact numerator");
    let denominator = value
        .column(2)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("validated exact denominator");
    let numerator = u128::from_le_bytes(
        numerator
            .value(row)
            .try_into()
            .map_err(|_| invalid("exact numerator requires 16 bytes"))?,
    );
    let denominator = u128::from_le_bytes(
        denominator
            .value(row)
            .try_into()
            .map_err(|_| invalid("exact denominator requires 16 bytes"))?,
    );
    let denominator =
        std::num::NonZeroU128::new(denominator).ok_or_else(|| invalid("zero exact denominator"))?;
    let exact = ExactNumber::new(sign.value(row), numerator, denominator);
    if exact.numerator != numerator
        || exact.denominator != denominator
        || exact.negative != sign.value(row)
    {
        return Err(invalid("noncanonical exact number"));
    }
    Ok(ScalarValue::Exact(exact))
}

pub(super) fn scalar_type(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Null
            | DataType::Boolean
            | DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
            | DataType::Utf8
            | DataType::Timestamp(TimeUnit::Nanosecond, _)
    ) || data_type == &exact_type()
}

pub(super) fn numeric_type(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
    ) || data_type == &exact_type()
}

pub(super) fn values_array(data_type: &DataType, values: &[ScalarValue]) -> Result<ArrayRef> {
    macro_rules! number {
        ($array:ty, $variant:ident, $native:ty) => {{
            let values = values
                .iter()
                .map(|value| match value {
                    ScalarValue::$variant(value) => <$native>::try_from(*value)
                        .ok()
                        .map(Some)
                        .ok_or_else(|| invalid("aggregate output width overflow")),
                    ScalarValue::Null | ScalarValue::Missing => Ok(None),
                    _ => Err(invalid("aggregate output type mismatch")),
                })
                .collect::<Result<Vec<_>>>()?;
            Arc::new(<$array>::from(values)) as ArrayRef
        }};
    }
    Ok(match data_type {
        DataType::Null => Arc::new(NullArray::new(values.len())),
        DataType::Int8 => number!(Int8Array, Signed, i8),
        DataType::Int16 => number!(Int16Array, Signed, i16),
        DataType::Int32 => number!(Int32Array, Signed, i32),
        DataType::Int64 => number!(Int64Array, Signed, i64),
        DataType::UInt8 => number!(UInt8Array, Unsigned, u8),
        DataType::UInt16 => number!(UInt16Array, Unsigned, u16),
        DataType::UInt32 => number!(UInt32Array, Unsigned, u32),
        DataType::UInt64 => number!(UInt64Array, Unsigned, u64),
        DataType::Boolean => Arc::new(BooleanArray::from(
            values
                .iter()
                .map(|value| match value {
                    ScalarValue::Bool(value) => Some(*value),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )),
        DataType::Float32 => Arc::new(Float32Array::from(
            values
                .iter()
                .map(|value| match value {
                    ScalarValue::Float(value) => Some(*value as f32),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )),
        DataType::Float64 => Arc::new(Float64Array::from(
            values
                .iter()
                .map(|value| match value {
                    ScalarValue::Float(value) => Some(*value),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )),
        DataType::Utf8 => Arc::new(StringArray::from(
            values
                .iter()
                .map(|value| match value {
                    ScalarValue::Text(value) => Some(value.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )),
        DataType::Timestamp(TimeUnit::Nanosecond, zone) => {
            let values = values
                .iter()
                .map(|value| match value {
                    ScalarValue::Datetime(value) => i64::try_from(*value)
                        .map(Some)
                        .map_err(|_| invalid("timestamp output overflow")),
                    ScalarValue::Null | ScalarValue::Missing => Ok(None),
                    _ => Err(invalid("timestamp output type mismatch")),
                })
                .collect::<Result<Vec<_>>>()?;
            Arc::new(TimestampNanosecondArray::from(values).with_timezone_opt(zone.clone()))
        }
        data_type if data_type == &exact_type() => {
            let values = values
                .iter()
                .map(|value| match value {
                    ScalarValue::Null | ScalarValue::Missing => Ok(None),
                    value => harmos_domain::model::storage::query::scalar::exact(value)
                        .map(Some)
                        .ok_or_else(|| invalid("exact output type mismatch")),
                })
                .collect::<Result<Vec<_>>>()?;
            let signs = BooleanArray::from(
                values
                    .iter()
                    .map(|value| value.is_some_and(|value| value.negative))
                    .collect::<Vec<_>>(),
            );
            let numerators = values
                .iter()
                .map(|value| value.map_or(0, |value| value.numerator).to_le_bytes())
                .collect::<Vec<_>>();
            let denominators = values
                .iter()
                .map(|value| {
                    value
                        .map_or(1, |value| value.denominator.get())
                        .to_le_bytes()
                })
                .collect::<Vec<_>>();
            let numerators =
                BinaryArray::from_iter_values(numerators.iter().map(|value| value.as_slice()));
            let denominators =
                BinaryArray::from_iter_values(denominators.iter().map(|value| value.as_slice()));
            let DataType::Struct(fields) = exact_type() else {
                unreachable!()
            };
            let nulls = arrow_buffer::NullBuffer::from(
                values.iter().map(Option::is_some).collect::<Vec<_>>(),
            );
            Arc::new(StructArray::new(
                fields,
                vec![
                    Arc::new(signs),
                    Arc::new(numerators),
                    Arc::new(denominators),
                ],
                Some(nulls),
            ))
        }
        _ => return Err(invalid("unsupported scalar output type")),
    })
}

/* === Tests === */
