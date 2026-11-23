/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Arrow Plumbing For Shared Statistics Kernels

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use arrow_array::{ArrayRef, BinaryArray, Float64Array, Int64Array, ListArray, RecordBatch, StringArray, UInt64Array, types::Float64Type};
use arrow_schema::{DataType, Field, Schema, TimeUnit};

// Local Imports
use super::downsample::number;
use super::{FieldPathExecution, Transform, invalid, numeric_type};
use crate::Result;
use crate::scalar::ScalarValue;
use crate::spill::{Builder, Spool};
use crate::statistics::buckets::{OrderedSummary, Point};
use crate::statistics::{self, Buckets, Quantiles};
use harmos_domain::model::storage::query::transform::statistics::{BoxSummary, BucketSummary};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(crate) fn box_schema(plan: &BoxSummary, input: &Schema) -> Result<Schema> {
    if !numeric_type(plan.field.field(input)?.data_type()) {
        return Err(invalid("select a numeric box-summary field"));
    }
    let mut fields = ["q1", "median", "q3", "lower_whisker", "upper_whisker"]
        .into_iter()
        .map(|name| Field::new(name, DataType::Float64, true))
        .collect::<Vec<_>>();
    fields.push(Field::new("outlier_count", DataType::UInt64, false));
    fields.push(Field::new("outliers", DataType::List(Arc::new(Field::new("item", DataType::Float64, true))), false));
    Ok(Schema::new(fields))
}

pub(crate) fn bucket_schema(plan: &BucketSummary, input: &Schema) -> Result<Schema> {
    let time = plan.time.field(input)?;
    if !matches!(time.data_type(), DataType::Int64 | DataType::Timestamp(TimeUnit::Nanosecond, _))
        || plan.position.field(input)?.data_type() != &DataType::UInt64
        || !numeric_type(plan.value.field(input)?.data_type())
    {
        return Err(invalid("select Int64 or nanosecond timestamp time, UInt64 source position and numeric value fields"));
    }
    let mut fields = vec![
        Field::new("ordinal", DataType::UInt64, true),
        Field::new("width_ns", DataType::Utf8, false),
        Field::new("count", DataType::UInt64, false),
    ];
    for name in ["sum", "mean", "sum_squares", "rms", "std"] {
        fields.push(Field::new(name, DataType::Float64, true));
    }
    for name in ["first", "last", "min", "max"] {
        fields.push(Field::new(format!("{name}_time_ns"), DataType::Int64, true));
        fields.push(Field::new(format!("{name}_position"), DataType::UInt64, true));
        fields.push(Field::new(format!("{name}_value"), DataType::Float64, true));
    }
    fields.push(Field::new("continuation", DataType::Binary, true));
    Ok(Schema::new(fields))
}

pub(crate) fn apply(plan: &Transform, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
    let mut builder = Builder::cached(input.schema())?;
    builder.push(input, check)?;
    reduce(builder.finish()?, plan, check)
}

pub(crate) fn reduce(input: Spool, plan: &Transform, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
    match plan {
        Transform::BoxSummary(plan) => box_summary(input, plan, check),
        Transform::BucketSummary(plan) => bucket_summary(input, plan, check),
        _ => unreachable!("statistics transform"),
    }
}

/* === Private Functions === */

fn box_summary(input: Spool, plan: &BoxSummary, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
    let schema = Arc::new(box_schema(plan, &input.schema)?);
    let mut quantiles = Quantiles::new(plan.memory_bytes);
    for batch in input.reader()? {
        check()?;
        let batch = batch.map_err(|_| invalid("recreate the unreadable box-summary input"))?;
        for row in 0..batch.num_rows() {
            check()?;
            let value = plan.field.value(&batch, row)?;
            if matches!(value, ScalarValue::Missing | ScalarValue::Null) {
                continue;
            }
            quantiles.push(number(&value)?)?;
        }
    }
    let mut summary = statistics::BoxSummary::new(quantiles, plan.max_outliers, plan.scratch_bytes, check)?;
    if let Some(summary) = &mut summary {
        for batch in input.reader()? {
            check()?;
            let batch = batch.map_err(|_| invalid("recreate the unreadable box-summary input"))?;
            for row in 0..batch.num_rows() {
                check()?;
                let value = plan.field.value(&batch, row)?;
                if !matches!(value, ScalarValue::Missing | ScalarValue::Null) {
                    summary.push(number(&value)?);
                }
            }
        }
    }
    let mut columns: Vec<ArrayRef> = (0..3)
        .map(|index| Arc::new(Float64Array::from(vec![summary.as_ref().map(|summary| summary.quartiles[index])])) as ArrayRef)
        .collect();
    columns.push(Arc::new(Float64Array::from(vec![summary.as_ref().and_then(|summary| summary.lower)])));
    columns.push(Arc::new(Float64Array::from(vec![summary.as_ref().and_then(|summary| summary.upper)])));
    columns.push(Arc::new(UInt64Array::from(vec![summary.as_ref().map_or(0, |summary| summary.outlier_count)])));
    let outliers = summary.map_or_else(Vec::new, |summary| summary.outliers);
    if outliers.len() > i32::MAX as usize {
        return Err(invalid("reduce the outlier bound to fit Arrow list offsets"));
    }
    columns.push(Arc::new(ListArray::from_iter_primitive::<Float64Type, _, _>([Some(outliers.into_iter().map(Some))])));
    RecordBatch::try_new(schema, columns).map_err(|_| invalid("reduce box-summary output size"))
}

fn bucket_summary(input: Spool, plan: &BucketSummary, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
    let schema = Arc::new(bucket_schema(plan, &input.schema)?);
    let mut buckets = Buckets::new(plan, check)?;
    for batch in input.reader()? {
        check()?;
        let batch = batch.map_err(|_| invalid("recreate the unreadable bucket-summary input"))?;
        for row in 0..batch.num_rows() {
            check()?;
            let time_ns = match plan.time.value(&batch, row)? {
                ScalarValue::Signed(value) => value,
                ScalarValue::Datetime(value) => i64::try_from(value).map_err(|_| invalid("supply a time within Int64 nanoseconds"))?,
                _ => return Err(invalid("supply non-null sample times")),
            };
            let ScalarValue::Unsigned(position) = plan.position.value(&batch, row)? else {
                return Err(invalid("supply non-null source positions"));
            };
            let value = plan.value.value(&batch, row)?;
            let value = if matches!(value, ScalarValue::Missing | ScalarValue::Null) { f64::NAN } else { number(&value)? };
            buckets.push(Point { time_ns, position, value }, check)?;
        }
    }
    let continuation = buckets.continuation()?;
    if continuation.len() > i32::MAX as usize {
        return Err(invalid("reduce the bucket bound to fit Arrow binary offsets"));
    }
    let rows: Vec<_> = if buckets.buckets.is_empty() {
        vec![(None, None)]
    } else {
        buckets.buckets.iter().map(|(ordinal, summary)| (Some(*ordinal), Some(summary))).collect()
    };
    let mut columns: Vec<ArrayRef> = vec![
        Arc::new(UInt64Array::from(rows.iter().map(|(ordinal, _)| *ordinal).collect::<Vec<_>>())),
        Arc::new(StringArray::from(vec![buckets.width_ns.to_string(); rows.len()])),
        Arc::new(UInt64Array::from(
            rows.iter().map(|(_, summary)| summary.map_or(0, |summary| summary.moments.count())).collect::<Vec<_>>(),
        )),
    ];
    for measure in 0..5 {
        let values = rows
            .iter()
            .map(|(_, summary)| {
                summary.and_then(|summary| match measure {
                    0 => Some(summary.moments.sum()),
                    1 => summary.moments.mean(),
                    2 => Some(summary.moments.squares()),
                    3 => summary.moments.rms(),
                    _ => summary.moments.std(),
                })
            })
            .collect::<Vec<_>>();
        if values.iter().flatten().any(|value| !value.is_finite()) {
            return Err(invalid("reduce the bucket range to keep output moments finite"));
        }
        columns.push(Arc::new(Float64Array::from(values)));
    }
    let selectors: [fn(&OrderedSummary) -> Point; 4] =
        [|summary| summary.first, |summary| summary.last, |summary| summary.min, |summary| summary.max];
    for select in selectors {
        let points = rows.iter().map(|(_, summary)| summary.map(select)).collect::<Vec<_>>();
        columns.push(Arc::new(Int64Array::from(points.iter().map(|point| point.map(|point| point.time_ns)).collect::<Vec<_>>())));
        columns.push(Arc::new(UInt64Array::from(points.iter().map(|point| point.map(|point| point.position)).collect::<Vec<_>>())));
        columns.push(Arc::new(Float64Array::from(points.iter().map(|point| point.map(|point| point.value)).collect::<Vec<_>>())));
    }
    columns.push(Arc::new(BinaryArray::from(
        (0..rows.len()).map(|index| (index == 0).then_some(continuation.as_slice())).collect::<Vec<_>>(),
    )));
    check()?;
    RecordBatch::try_new(schema, columns).map_err(|_| invalid("reduce bucket-summary output size"))
}

/* === Tests === */
