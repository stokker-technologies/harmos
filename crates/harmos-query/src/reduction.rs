/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exact Paged Reductions over Private Spill Files

/* === Modules === */

#[cfg(feature = "testing")]
pub(super) mod testing;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::sync::Arc;

// Third Party Imports
use arrow_array::{Array, Float64Array, RecordBatch};

// Local Imports
use super::scalar::ScalarValue;
use super::spill::{Builder, Spool, io};
use super::transform::aggregate::Aggregate;
use super::transform::downsample::{Algorithm, Downsample, number};
use super::transform::{AggregateExecution, FieldPathExecution, HistogramExecution, Transform, TransformExecution, invalid};
use super::transform::{
    ProjectExecution,
    project::{Project, Projection},
};
use crate::Result;

/* === Constants === */

const COORDINATES: usize = 4096;
const COORDINATE_CACHE_BYTES: usize = 32 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Each Coordinate Keeps Its Source Row; Rows Without a Coordinate Are Skipped.
struct Coordinates {
    file: Option<File>,
    length: usize,
    start: usize,
    cache: Vec<(f64, f64, usize)>,
}

struct Selection {
    file: File,
    last: Option<usize>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Coordinates {
    fn get(&mut self, index: usize) -> Result<(f64, f64, usize)> {
        if index < self.start || index >= self.start + self.cache.len() {
            self.start = index / COORDINATES * COORDINATES;
            self.file
                .as_mut()
                .expect("uncached coordinates own a file")
                .seek(SeekFrom::Start((self.start as u64).checked_mul(24).ok_or_else(|| invalid("coordinate offset overflow"))?))
                .map_err(io)?;
            let count = COORDINATES.min(self.length - self.start);
            self.cache.clear();
            let mut bytes = vec![0; count * 24];
            self.file.as_mut().unwrap().read_exact(&mut bytes).map_err(io)?;
            for bytes in bytes.as_chunks::<24>().0 {
                let word = |at: usize| <[u8; 8]>::try_from(&bytes[at..at + 8]).unwrap();
                self.cache.push((f64::from_le_bytes(word(0)), f64::from_le_bytes(word(8)), u64::from_le_bytes(word(16)) as usize));
            }
        }
        Ok(self.cache[index - self.start])
    }

    fn mean(&mut self, start: usize, end: usize, check: &mut impl FnMut() -> Result<()>) -> Result<(f64, f64)> {
        if start == end {
            let (x, y, _) = self.get(self.length - 1)?;
            return Ok((x, y));
        }
        let (mut x, mut y) = (0.0, 0.0);
        for index in start..end {
            check()?;
            let point = self.get(index)?;
            x += point.0;
            y += point.1;
        }
        Ok((x / (end - start) as f64, y / (end - start) as f64))
    }

    fn partition(&mut self, edge: f64) -> Result<usize> {
        let (mut low, mut high) = (0, self.length);
        while low < high {
            let middle = low + (high - low) / 2;
            if self.get(middle)?.0 < edge {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low)
    }
}

impl Selection {
    fn push(&mut self, index: usize) -> Result<()> {
        if self.last != Some(index) {
            self.file.write_all(&(index as u64).to_le_bytes()).map_err(io)?;
            self.last = Some(index);
        }
        Ok(())
    }
}

/* === Public Functions === */

pub(super) fn transform(input: Spool, plan: &Transform, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
    match plan {
        Transform::BoxSummary(_) | Transform::BucketSummary(_) => {
            let batch = super::transform::statistics::reduce(input, plan, check)?;
            let mut output = Builder::new(batch.schema())?;
            output.push(&batch, check)?;
            output.finish()
        }
        Transform::TimeBuckets(plan) => {
            let mut accumulator = super::time_buckets::Accumulator::new(plan, input.schema.clone())?;
            for batch in input.reader()? {
                check()?;
                accumulator.push(&batch.map_err(|_| invalid("bucket spill decoding failed"))?, check)?;
            }
            let batch = accumulator.finish(check)?;
            let mut output = Builder::new(batch.schema())?;
            output.push(&batch, check)?;
            output.finish()
        }
        Transform::Sort(plan) => super::ordering::sort(input, plan, check),
        Transform::Downsample(plan) => downsample(input, plan, check),
        Transform::Aggregate(plan) => aggregate(input, plan, check),
        Transform::Histogram(plan) => {
            let mut bins = plan.start(&input.schema)?;
            for batch in input.reader()? {
                check()?;
                bins.push(&batch.map_err(|_| invalid("histogram spill decoding failed"))?, check)?;
            }
            let batch = bins.finish()?;
            let mut output = Builder::new(batch.schema())?;
            output.push(&batch, check)?;
            output.finish()
        }
        _ => {
            let schema = Arc::new(plan.output_schema(&input.schema)?);
            let mut output = Builder::new(schema)?;
            for batch in input.reader()? {
                check()?;
                let batch = batch.map_err(|_| invalid("transform spill decoding failed"))?;
                output.push(&plan.apply(&batch, check)?, check)?;
            }
            output.finish()
        }
    }
}

pub(super) fn downsample(input: Spool, plan: &Downsample, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
    if input.rows <= plan.max_rows {
        return Ok(input);
    }
    let mut selected = Selection {
        file: tempfile::tempfile().map_err(io)?,
        last: None,
    };
    if matches!(plan.algorithm, Algorithm::Decimate) {
        let stride = (input.rows as f64 / plan.max_rows as f64).ceil() as usize;
        for index in (0..input.rows).step_by(stride.max(1)) {
            check()?;
            selected.push(index)?;
        }
    } else {
        let mut coordinates = coordinates(&input, plan, COORDINATE_CACHE_BYTES, check)?;
        let length = coordinates.length;
        if length <= plan.max_rows {
            for index in 0..length {
                check()?;
                selected.push(coordinates.get(index)?.2)?;
            }
        } else if matches!(plan.algorithm, Algorithm::Extrema) {
            let span = ((length as f64 / (plan.max_rows / 2) as f64).ceil() as usize).max(1);
            for start in (0..length).step_by(span) {
                let (mut low, mut high) = (start, start);
                let (mut minimum, mut maximum) = (coordinates.get(start)?.1, coordinates.get(start)?.1);
                for index in start + 1..start.saturating_add(span).min(length) {
                    check()?;
                    let value = coordinates.get(index)?.1;
                    if value < minimum {
                        low = index;
                        minimum = value;
                    }
                    if value > maximum {
                        high = index;
                        maximum = value;
                    }
                }
                selected.push(coordinates.get(low.min(high))?.2)?;
                selected.push(coordinates.get(low.max(high))?.2)?;
            }
        } else {
            triangles(&mut coordinates, plan, &mut selected, check)?;
        }
    }
    selected.file.seek(SeekFrom::Start(0)).map_err(io)?;
    let mut indices = BufReader::new(selected.file);
    let mut next = read_index(&mut indices)?;
    let mut output = Builder::cached(input.schema.clone())?;
    let mut offset = 0usize;
    for batch in input.reader()? {
        check()?;
        let batch = batch.map_err(|_| invalid("downsample spill decoding failed"))?;
        let end = offset.checked_add(batch.num_rows()).ok_or_else(|| invalid("downsample position overflow"))?;
        let mut local = Vec::new();
        while let Some(index) = next {
            if index >= end {
                break;
            }
            local.push((index - offset) as u64);
            next = read_index(&mut indices)?;
        }
        if !local.is_empty() {
            output.push(&super::transform::take(&batch, local)?, check)?;
        }
        offset = end;
    }
    output.finish()
}

/* === Private Functions === */

fn aggregate(input: Spool, plan: &Aggregate, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
    let schema = Arc::new(plan.output_schema(&input.schema)?);
    let mut keys = |batch: &RecordBatch, row| group_keys(plan, batch, row);
    let input = if plan.groups.is_empty() {
        input
    } else {
        super::ordering::ordered(input, &mut keys, &mut |left, right| compare_keys(left, right), check)?
    };

    // Ordered Rows Fold In Runs Of Equal Keys; Each Run Is One Contiguous Slice.
    let mut output = Builder::new(schema)?;
    let mut groups = Some(plan.start(&input.schema)?);
    let mut previous: Option<Vec<ScalarValue>> = None;
    for batch in input.reader()? {
        let batch = batch.map_err(|_| invalid("aggregate spill decoding failed"))?;
        check()?;
        let mut start = 0;
        for row in (0..batch.num_rows()).filter(|_| !plan.groups.is_empty()) {
            check()?;
            let current = keys(&batch, row)?;
            let changed = match &previous {
                Some(previous) => !compare_keys(previous, &current)?.is_eq(),
                None => false,
            };
            if changed {
                groups.as_mut().unwrap().push(&batch.slice(start, row - start), check)?;
                start = row;
                let finished = groups.take().unwrap().finish(check)?;
                output.push(&finished, check)?;
                groups = Some(plan.start(&input.schema)?);
            }
            previous = Some(current);
        }
        groups.as_mut().unwrap().push(&batch.slice(start, batch.num_rows() - start), check)?;
    }
    output.push(&groups.take().unwrap().finish(check)?, check)?;
    output.finish()
}

fn group_keys(plan: &Aggregate, batch: &RecordBatch, row: usize) -> Result<Vec<ScalarValue>> {
    plan.groups.iter().map(|group| group.field.value(batch, row)).collect()
}

fn compare_keys(left: &[ScalarValue], right: &[ScalarValue]) -> Result<Ordering> {
    for (left, right) in left.iter().zip(right) {
        let order = left.compare(right)?;
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(Ordering::Equal)
}

fn coordinates(input: &Spool, plan: &Downsample, cache_bytes: usize, check: &mut impl FnMut() -> Result<()>) -> Result<Coordinates> {
    let cached = input.rows <= cache_bytes / 24;
    let mut writer = if cached { None } else { Some(BufWriter::with_capacity(1024 * 1024, tempfile::tempfile().map_err(io)?)) };
    let mut cache = Vec::with_capacity(if cached { input.rows } else { 0 });
    let mut first_time = None;
    let mut previous = None;
    let (mut offset, mut length) = (0usize, 0usize);
    let field = plan.value.as_ref().ok_or_else(|| invalid("missing downsample value field"))?;
    let mut fields = vec![Projection { field: field.clone(), name: "y".into() }];
    if let Some(axis) = &plan.axis
        && !matches!(plan.algorithm, Algorithm::Extrema)
    {
        fields.push(Projection { field: axis.clone(), name: "x".into() });
    }
    let projection = Project { fields };
    for batch in input.reader()? {
        let batch = batch.map_err(|_| invalid("coordinate spill decoding failed"))?;
        // Resolve Paths And Parent Null Masks Once Per Batch.
        let columns = projection.apply(&batch, check)?;
        for row in 0..batch.num_rows() {
            check()?;
            let source = offset + row;
            let y = coordinate(columns.column(0).as_ref(), row)?;
            let x = match &plan.axis {
                _ if matches!(plan.algorithm, Algorithm::Extrema) => Some(source as f64),
                None => Some(source as f64),
                Some(_) => match super::transform::scalar(columns.column(1).as_ref(), row)? {
                    ScalarValue::Datetime(value) => {
                        let first = *first_time.get_or_insert(value);
                        Some(value.checked_sub(first).ok_or_else(|| invalid("LTTB timestamp difference overflow"))? as f64)
                    }
                    ScalarValue::Null | ScalarValue::Missing => None,
                    value => Some(number(&value)?),
                },
            };
            let (Some(x), Some(y)) = (x, y) else {
                continue;
            };
            if matches!(plan.algorithm, Algorithm::LttbLog) && (x <= 0.0 || previous.is_some_and(|previous| previous > x)) {
                return Err(invalid("logarithmic LTTB requires positive ordered coordinates"));
            }
            if let Some(writer) = &mut writer {
                writer.write_all(&x.to_le_bytes()).map_err(io)?;
                writer.write_all(&y.to_le_bytes()).map_err(io)?;
                writer.write_all(&(source as u64).to_le_bytes()).map_err(io)?;
            } else {
                cache.push((x, y, source));
            }
            previous = Some(x);
            length += 1;
        }
        offset += batch.num_rows();
    }
    let file = writer.map(|writer| writer.into_inner().map_err(|error| io(error.into_error()))).transpose()?;
    Ok(Coordinates { file, length, start: 0, cache })
}

// A Missing or Nonfinite Coordinate Is Skipped.
fn coordinate(array: &dyn Array, row: usize) -> Result<Option<f64>> {
    if let Some(values) = array.as_any().downcast_ref::<Float64Array>() {
        return Ok(Some(values.value(row)).filter(|value| !values.is_null(row) && value.is_finite()));
    }
    match super::transform::scalar(array, row)? {
        ScalarValue::Null | ScalarValue::Missing => Ok(None),
        value => number(&value).map(Some),
    }
}

fn triangles(points: &mut Coordinates, plan: &Downsample, selected: &mut Selection, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
    let length = points.length;
    let count = plan.max_rows;
    let first = points.get(0)?.0.max(f64::MIN_POSITIVE);
    let last = points.get(length - 1)?.0;
    let low = first.ln();
    let span = last.ln() - low;
    let logarithmic = matches!(plan.algorithm, Algorithm::LttbLog) && last.is_finite() && last > first && span > 0.0;
    let buckets = count - 2;
    let bucket_width = (length - 2) as f64 / buckets as f64;
    let edge = |bucket: usize| (low + (bucket as f64 / buckets as f64) * span).exp();
    selected.push(points.get(0)?.2)?;
    let mut previous = 0;
    for bucket in 0..buckets {
        check()?;
        let (start, end, ahead) = if logarithmic {
            let start = points.partition(edge(bucket))?.min(length - 1);
            let end = points.partition(edge(bucket + 1))?.clamp(start + 1, length);
            let ahead = if bucket < buckets - 1 { points.partition(edge(bucket + 2).min(last))?.clamp(end, length) } else { length };
            (start, end, ahead)
        } else {
            let start = 1 + (bucket as f64 * bucket_width).floor() as usize;
            let end = (1 + ((bucket + 1) as f64 * bucket_width).floor() as usize).min(length - 1);
            let ahead = if bucket < buckets - 1 { (1 + ((bucket + 2) as f64 * bucket_width).floor() as usize).min(length) } else { length };
            (start, end, ahead)
        };
        let (mut mean_x, mean_y) = points.mean(end, ahead, check)?;
        let (mut previous_x, previous_y, _) = points.get(previous)?;
        if logarithmic {
            previous_x = previous_x.max(f64::MIN_POSITIVE).ln();
            mean_x = mean_x.max(f64::MIN_POSITIVE).ln();
        }
        let mut picked = start;
        let mut largest: Option<f64> = None;
        for index in start..end {
            check()?;
            let (mut x, y, _) = points.get(index)?;
            if logarithmic {
                x = x.max(f64::MIN_POSITIVE).ln();
            }
            let area = ((previous_x - mean_x) * (y - previous_y) - (previous_x - x) * (mean_y - previous_y)).abs();
            // Iterator::max_by Selects The Last Equal Candidate.
            if largest.is_none_or(|largest| !area.total_cmp(&largest).is_lt()) {
                picked = index;
                largest = Some(area);
            }
        }
        selected.push(points.get(picked)?.2)?;
        previous = picked;
    }
    selected.push(points.get(length - 1)?.2)
}

fn read_index(reader: &mut impl Read) -> Result<Option<usize>> {
    let mut bytes = [0; 8];
    let count = reader.read(&mut bytes[..1]).map_err(io)?;
    if count == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut bytes[1..]).map_err(io)?;
    usize::try_from(u64::from_le_bytes(bytes)).map(Some).map_err(|_| invalid("selected row exceeds platform size"))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::super::transform::{
        FieldPath,
        aggregate::{Aggregate, Measure},
        project::Projection,
    };
    use super::*;
    use arrow_array::{Int64Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};

    #[test]
    fn spilled_grouped_statistics_match_stable_eager_reduction() {
        let schema = Arc::new(Schema::new(vec![Field::new("group", DataType::Int64, false), Field::new("value", DataType::Int64, false)]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int64Array::from((0..1001).map(|i| i % 7).collect::<Vec<i64>>())),
                Arc::new(Int64Array::from((0..1001).map(|i| (i * 17 % 101) - 50).collect::<Vec<i64>>())),
            ],
        )
        .unwrap();
        let plan = Aggregate {
            groups: vec![Projection {
                field: FieldPath::from("group"),
                name: "group".into(),
            }],
            measures: vec![
                Measure {
                    field: FieldPath::from("value"),
                    statistic: crate::Aggregate::Median,
                    name: "median".into(),
                },
                Measure {
                    field: FieldPath::from("value"),
                    statistic: crate::Aggregate::Sum,
                    name: "sum".into(),
                },
                Measure {
                    field: FieldPath::from("value"),
                    statistic: crate::Aggregate::Percentile(77),
                    name: "percentile".into(),
                },
            ],
        };
        let expected = plan.apply(&batch, &mut || Ok(())).unwrap();
        let mut spool = Builder::new(schema).unwrap();
        for offset in (0..batch.num_rows()).step_by(31) {
            spool.push(&batch.slice(offset, 31.min(batch.num_rows() - offset)), &mut || Ok(())).unwrap();
        }
        let actual = transform(spool.finish().unwrap(), &Transform::Aggregate(plan), &mut || Ok(())).unwrap();
        let mut pages = actual.pages().unwrap();
        let mut output = Vec::new();
        while let Some(batch) = pages.next().unwrap() {
            output.push(batch);
        }
        let combined = arrow_select::concat::concat_batches(&expected.schema(), output.iter()).unwrap();
        assert_eq!(combined, expected);
    }

    #[test]
    fn downsampling_skips_missing_and_nonfinite_coordinates() {
        let schema = Arc::new(Schema::new(vec![Field::new("value", DataType::Float64, true)]));
        let values = (0..10_000).map(|row| match row % 1000 {
            0..100 => None,
            100..200 => Some(f64::NAN),
            200..210 => Some(f64::INFINITY),
            _ => Some((row as f64 / 50.0).sin()),
        });
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(Float64Array::from_iter(values))]).unwrap();
        for algorithm in [Algorithm::Lttb, Algorithm::Extrema] {
            let plan = Downsample {
                algorithm,
                max_rows: 100,
                value: Some(FieldPath::from("value")),
                axis: None,
            };
            let mut spool = Builder::new(schema.clone()).unwrap();
            spool.push(&batch, &mut || Ok(())).unwrap();
            let mut pages = downsample(spool.finish().unwrap(), &plan, &mut || Ok(())).unwrap().pages().unwrap();
            let mut kept = Vec::new();
            while let Some(page) = pages.next().unwrap() {
                kept.extend(page.column(0).as_any().downcast_ref::<Float64Array>().unwrap().iter());
            }
            assert!((2..=100).contains(&kept.len()) && kept.iter().all(|value| value.is_some_and(f64::is_finite)));
        }
    }
}
