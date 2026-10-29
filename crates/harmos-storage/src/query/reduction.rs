/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exact Paged Reductions Over Private Spill Files

use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::sync::Arc;

use arrow_array::{Array, Float64Array, RecordBatch};

use super::scalar::ScalarValue;
use super::spill::{Builder, Spool, io};
use super::transform::downsample::{Algorithm, Downsample, number};
use super::transform::{AggregateExecution, FieldPathExecution, HistogramExecution, Transform, TransformExecution, invalid};
use super::transform::{
    ProjectExecution,
    project::{Project, Projection},
};
use crate::Result;

const COORDINATES: usize = 4096;
const COORDINATE_CACHE_BYTES: usize = 32 * 1024 * 1024;

struct Coordinates {
    file: Option<File>,
    length: usize,
    start: usize,
    cache: Vec<(f64, f64)>,
}

struct Selection {
    file: File,
    last: Option<usize>,
}

impl Coordinates {
    fn get(&mut self, index: usize) -> Result<(f64, f64)> {
        if index < self.start || index >= self.start + self.cache.len() {
            self.start = index / COORDINATES * COORDINATES;
            self.file
                .as_mut()
                .expect("uncached coordinates own a file")
                .seek(SeekFrom::Start((self.start as u64).checked_mul(16).ok_or_else(|| invalid("coordinate offset overflow"))?))
                .map_err(io)?;
            let count = COORDINATES.min(self.length - self.start);
            self.cache.clear();
            let mut bytes = vec![0; count * 16];
            self.file.as_mut().unwrap().read_exact(&mut bytes).map_err(io)?;
            for bytes in bytes.as_chunks::<16>().0 {
                self.cache.push((f64::from_le_bytes(bytes[..8].try_into().unwrap()), f64::from_le_bytes(bytes[8..].try_into().unwrap())));
            }
        }
        Ok(self.cache[index - self.start])
    }

    fn mean(&mut self, start: usize, end: usize, check: &mut impl FnMut() -> Result<()>) -> Result<(f64, f64)> {
        if start == end {
            return self.get(self.length - 1);
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

pub(super) fn transform(input: Spool, plan: &Transform, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
    match plan {
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

fn normalized(value: ScalarValue) -> ScalarValue {
    if matches!(&value, ScalarValue::Float(value) if !value.is_finite()) { ScalarValue::Null } else { value }
}

fn aggregate(input: Spool, plan: &super::transform::aggregate::Aggregate, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
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

fn group_keys(plan: &super::transform::aggregate::Aggregate, batch: &RecordBatch, row: usize) -> Result<Vec<ScalarValue>> {
    plan.groups.iter().map(|group| group.field.value(batch, row).map(normalized)).collect()
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

pub(super) fn downsample(input: Spool, plan: &Downsample, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
    if input.rows <= plan.max_rows {
        return Ok(input);
    }
    let mut selected = Selection {
        file: tempfile::tempfile().map_err(io)?,
        last: None,
    };
    if matches!(plan.algorithm, Algorithm::Decimate) {
        for index in crate::signal::decimate_indices(input.rows, plan.max_rows) {
            check()?;
            selected.push(index)?;
        }
    } else {
        let mut coordinates = coordinates(&input, plan, COORDINATE_CACHE_BYTES, check)?;
        if matches!(plan.algorithm, Algorithm::Extrema) {
            let span = ((input.rows as f64 / (plan.max_rows / 2) as f64).ceil() as usize).max(1);
            for start in (0..input.rows).step_by(span) {
                let (mut low, mut high) = (start, start);
                let (mut minimum, mut maximum) = (coordinates.get(start)?.1, coordinates.get(start)?.1);
                for index in start + 1..start.saturating_add(span).min(input.rows) {
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
                selected.push(low.min(high))?;
                selected.push(low.max(high))?;
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

fn coordinates(input: &Spool, plan: &Downsample, cache_bytes: usize, check: &mut impl FnMut() -> Result<()>) -> Result<Coordinates> {
    let cached = input.rows <= cache_bytes / 16;
    let mut writer = if cached { None } else { Some(BufWriter::with_capacity(1024 * 1024, tempfile::tempfile().map_err(io)?)) };
    let mut cache = Vec::with_capacity(if cached { input.rows } else { 0 });
    let mut first_time = None;
    let mut previous = None;
    let mut index = 0usize;
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
            let y = coordinate(columns.column(0).as_ref(), row)?;
            let x = match &plan.axis {
                _ if matches!(plan.algorithm, Algorithm::Extrema) => index as f64,
                None => index as f64,
                Some(_) => match super::transform::scalar(columns.column(1).as_ref(), row)? {
                    ScalarValue::Datetime(value) => {
                        let first = *first_time.get_or_insert(value);
                        value.checked_sub(first).ok_or_else(|| invalid("LTTB timestamp difference overflow"))? as f64
                    }
                    value => number(&value)?,
                },
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(invalid("downsample requires finite non-null coordinates"));
            }
            if matches!(plan.algorithm, Algorithm::LttbLog) && (x <= 0.0 || previous.is_some_and(|previous| previous > x)) {
                return Err(invalid("logarithmic LTTB requires positive ordered coordinates"));
            }
            if let Some(writer) = &mut writer {
                writer.write_all(&x.to_le_bytes()).map_err(io)?;
                writer.write_all(&y.to_le_bytes()).map_err(io)?;
            } else {
                cache.push((x, y));
            }
            previous = Some(x);
            index += 1;
        }
    }
    let file = writer.map(|writer| writer.into_inner().map_err(|error| io(error.into_error()))).transpose()?;
    Ok(Coordinates {
        file,
        length: input.rows,
        start: 0,
        cache,
    })
}

fn coordinate(array: &dyn Array, row: usize) -> Result<f64> {
    if let Some(values) = array.as_any().downcast_ref::<Float64Array>()
        && !values.is_null(row)
    {
        return Ok(values.value(row));
    }
    number(&super::transform::scalar(array, row)?)
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
    selected.push(0)?;
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
        let (mut previous_x, previous_y) = points.get(previous)?;
        if logarithmic {
            previous_x = previous_x.max(f64::MIN_POSITIVE).ln();
            mean_x = mean_x.max(f64::MIN_POSITIVE).ln();
        }
        let mut picked = start;
        let mut largest: Option<f64> = None;
        for index in start..end {
            check()?;
            let (mut x, y) = points.get(index)?;
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
        selected.push(picked)?;
        previous = picked;
    }
    selected.push(length - 1)
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
    fn cached_and_disk_coordinates_preserve_timestamps_and_bucket_selection() {
        use arrow_array::TimestampNanosecondArray;
        use arrow_schema::TimeUnit;
        let schema = Arc::new(Schema::new(vec![
            Field::new("time", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
            Field::new("value", DataType::Int64, false),
        ]));
        let rows = 9001;
        let timestamps = TimestampNanosecondArray::from((0..rows).map(|row| i64::MAX - 20_000 + row).collect::<Vec<_>>());
        let values = Int64Array::from((0..rows).map(|row| row * 7919 % 97).collect::<Vec<_>>());
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(timestamps), Arc::new(values)]).unwrap();
        let plan = Downsample {
            algorithm: Algorithm::Lttb,
            max_rows: 103,
            value: Some(FieldPath::from("value")),
            axis: Some(FieldPath::from("time")),
        };
        let mut builder = Builder::cached(schema).unwrap();
        builder.push(&batch, &mut || Ok(())).unwrap();
        let spool = builder.finish().unwrap();
        let mut cached = coordinates(&spool, &plan, COORDINATE_CACHE_BYTES, &mut || Ok(())).unwrap();
        let mut disk = coordinates(&spool, &plan, 0, &mut || Ok(())).unwrap();
        assert!(cached.file.is_none());
        assert!(disk.file.is_some());
        for index in [0, 4095, 4096, 8192, 9000, 2, 5000] {
            assert_eq!(cached.get(index).unwrap(), disk.get(index).unwrap());
            assert_eq!(cached.get(index).unwrap().0, index as f64);
        }
        let x = (0..rows).map(|row| row as f64).collect::<Vec<_>>();
        let y = (0..rows).map(|row| (row * 7919 % 97) as f64).collect::<Vec<_>>();
        let expected = crate::signal::lttb(&x, &y, plan.max_rows);
        for mut coordinates in [cached, disk] {
            let mut selected = Selection {
                file: tempfile::tempfile().unwrap(),
                last: None,
            };
            triangles(&mut coordinates, &plan, &mut selected, &mut || Ok(())).unwrap();
            selected.file.rewind().unwrap();
            let mut actual = Vec::new();
            while let Some(index) = read_index(&mut selected.file).unwrap() {
                actual.push(index);
            }
            assert_eq!(actual, expected);
        }
    }

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
}
