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

pub use harmos_domain::model::storage::query::transform::TimeBucketPlan;

/* === Imports === */

// Standard Library Imports
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

// Third Party Imports
use arrow_array::{
    Array, ArrayRef, BinaryArray, Float64Array, Int64Array, ListArray, RecordBatch, StringArray, TimestampNanosecondArray, UInt64Array,
    types::UInt64Type,
};
use arrow_schema::{DataType, Field, Schema, TimeUnit};
use sha2::{Digest, Sha256};

// Local Imports
use super::{
    columnar, index,
    scalar::ScalarValue,
    transform::{FieldPath, FieldPathExecution},
};
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Accumulator {
    plan: TimeBucketPlan,
    schema: Arc<Schema>,
    samples: Vec<Sample>,
    owned_bytes: usize,
}

struct Sample {
    identity: Vec<u8>,
    key: Vec<u8>,
    bucket: i64,
    position: u64,
    time_ns: i64,
    value: f64,
}

struct Bucket {
    key: Vec<u8>,
    bucket: i64,
    revision: [u8; 32],
    count: u64,
    first: f64,
    last: f64,
    min: f64,
    max: f64,
    sum: f64,
    sum_squares: f64,
    mean: f64,
    m2: f64,
    first_position: u64,
    last_position: u64,
    min_position: u64,
    max_position: u64,
    positions: Vec<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl PartialEq for Sample {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.key == other.key
            && self.bucket == other.bucket
            && self.position == other.position
            && self.time_ns == other.time_ns
            && self.value.to_bits() == other.value.to_bits()
    }
}

impl Accumulator {
    pub fn new(plan: &TimeBucketPlan, input: Arc<Schema>) -> Result<Self> {
        validate(plan, &input)?;
        Ok(Self {
            plan: plan.clone(),
            schema: input,
            samples: vec![],
            owned_bytes: 0,
        })
    }

    pub fn memory_bytes(&self) -> usize {
        self.plan
            .memory_size()
            .saturating_add(self.samples.capacity().saturating_mul(std::mem::size_of::<Sample>()))
            .saturating_add(self.owned_bytes)
    }

    pub fn push(&mut self, batch: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        check()?;
        if batch.schema() != self.schema {
            return Err(columnar::invalid("time bucket input schema changed"));
        }
        let needed = self.samples.len().checked_add(batch.num_rows()).ok_or_else(columnar::exhausted)?;
        if needed > self.plan.max_samples.get() {
            return Err(columnar::exhausted());
        }
        if needed > self.samples.capacity() {
            let capacity = needed.max(self.samples.capacity().saturating_mul(2)).min(self.plan.max_samples.get());
            let bytes = capacity
                .saturating_mul(std::mem::size_of::<Sample>())
                .saturating_add(self.owned_bytes)
                .saturating_add(self.plan.memory_size());
            if bytes > self.plan.max_bytes.get() {
                return Err(columnar::exhausted());
            }
            self.samples.try_reserve_exact(capacity - self.samples.len()).map_err(|_| columnar::exhausted())?;
        }
        let positions = array(batch, "_storage.position")?;
        let positions = positions
            .as_any()
            .downcast_ref::<UInt64Array>()
            .ok_or_else(|| columnar::invalid("time buckets require UInt64 source positions"))?;
        let times = array(batch, &self.plan.time)?;
        let values = array(batch, &self.plan.value)?;
        let values =
            values.as_any().downcast_ref::<Float64Array>().ok_or_else(|| columnar::invalid("time bucket values must be Float64"))?;
        let identities = array(batch, &self.plan.identity)?;
        let groups = self.plan.group.iter().map(|name| array(batch, name)).collect::<Result<Vec<_>>>()?;
        for row in 0..batch.num_rows() {
            if row % 256 == 0 {
                check()?;
            }
            if identities.is_null(row)
                || positions.is_null(row)
                || values.is_null(row)
                || !values.value(row).is_finite()
                || positions.value(row) == 0
            {
                return Err(columnar::invalid("time buckets require finite values and non-null identities and positions"));
            }
            let time_ns = time(&times, row)?;
            let ordinal = (i128::from(time_ns) - i128::from(self.plan.origin_ns)).div_euclid(i128::from(self.plan.width_ns.get()));
            let bucket = i64::try_from(ordinal).map_err(|_| columnar::invalid("time bucket ordinal overflow"))?;
            let remaining = self.plan.max_bytes.get().saturating_sub(self.memory_bytes());
            let identity = scalar_key(&identities, row, remaining)?;
            let mut key = Vec::new();
            for group in &groups {
                let encoded = scalar_key(group, row, remaining.saturating_sub(identity.capacity()).saturating_sub(key.capacity()))?;
                if key.len().saturating_add(encoded.len()).saturating_add(identity.capacity()) > remaining {
                    return Err(columnar::exhausted());
                }
                key.try_reserve_exact(encoded.len()).map_err(|_| columnar::exhausted())?;
                key.extend(encoded);
            }
            let ordinal = index::encode_key(&[ScalarValue::Signed(bucket)])?;
            if key.len().saturating_add(ordinal.len()).saturating_add(identity.capacity()) > remaining {
                return Err(columnar::exhausted());
            }
            key.try_reserve_exact(ordinal.len()).map_err(|_| columnar::exhausted())?;
            key.extend(ordinal);
            self.owned_bytes = self.owned_bytes.saturating_add(identity.capacity()).saturating_add(key.capacity());
            if self.memory_bytes() > self.plan.max_bytes.get() {
                return Err(columnar::exhausted());
            }
            self.samples.push(Sample {
                identity,
                key,
                bucket,
                position: positions.value(row),
                time_ns,
                value: values.value(row),
            });
        }
        Ok(())
    }

    pub fn finish(mut self, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        check()?;
        self.samples.sort_unstable_by_key(|sample| sample.position);
        for pair in self.samples.windows(2) {
            if pair[0].position == pair[1].position && pair[0] != pair[1] {
                return Err(columnar::invalid("duplicate source position has conflicting bucket input"));
            }
        }
        self.samples.dedup_by_key(|sample| sample.position);
        check()?;
        self.samples.sort_unstable_by(|left, right| left.identity.cmp(&right.identity).then(right.position.cmp(&left.position)));
        self.samples.dedup_by(|left, right| left.identity == right.identity);
        check()?;
        self.samples
            .sort_unstable_by(|left, right| (&left.key, left.time_ns, left.position).cmp(&(&right.key, right.time_ns, right.position)));
        let count = self.samples.chunk_by(|left, right| left.key == right.key).count();
        let key_bytes = self.samples.chunk_by(|left, right| left.key == right.key).map(|group| group[0].key.len()).sum::<usize>();
        let bucket_bytes = count
            .saturating_mul(std::mem::size_of::<Bucket>())
            .saturating_add(key_bytes)
            .saturating_add(self.samples.len().saturating_mul(8));
        if self.memory_bytes().saturating_add(bucket_bytes) > self.plan.max_bytes.get() {
            return Err(columnar::exhausted());
        }
        let mut buckets = Vec::new();
        buckets.try_reserve_exact(count).map_err(|_| columnar::exhausted())?;
        for group in self.samples.chunk_by(|left, right| left.key == right.key) {
            check()?;
            let first = &group[0];
            let mut bucket = Bucket {
                key: first.key.clone(),
                bucket: first.bucket,
                revision: [0; 32],
                count: 0,
                first: first.value,
                last: first.value,
                min: first.value,
                max: first.value,
                sum: 0.0,
                sum_squares: 0.0,
                mean: 0.0,
                m2: 0.0,
                first_position: first.position,
                last_position: first.position,
                min_position: first.position,
                max_position: first.position,
                positions: Vec::new(),
            };
            bucket.positions.try_reserve_exact(group.len()).map_err(|_| columnar::exhausted())?;
            for (index, sample) in group.iter().enumerate() {
                if index % 256 == 0 {
                    check()?;
                }
                bucket.count += 1;
                bucket.last = sample.value;
                bucket.last_position = sample.position;
                if sample.value < bucket.min {
                    bucket.min = sample.value;
                    bucket.min_position = sample.position;
                }
                if sample.value > bucket.max {
                    bucket.max = sample.value;
                    bucket.max_position = sample.position;
                }
                bucket.sum += sample.value;
                bucket.sum_squares += sample.value * sample.value;
                let delta = sample.value - bucket.mean;
                bucket.mean += delta / bucket.count as f64;
                bucket.m2 += delta * (sample.value - bucket.mean);
                if [bucket.sum, bucket.sum_squares, bucket.mean, bucket.m2].iter().any(|value| !value.is_finite()) {
                    return Err(columnar::invalid("time bucket moments overflowed"));
                }
                bucket.positions.push(sample.position);
            }
            bucket.revision = revision(&bucket);
            buckets.push(bucket);
        }
        let output_bound = count
            .saturating_mul(256)
            .saturating_add(key_bytes.saturating_mul(2))
            .saturating_add(self.samples.len().saturating_mul(16))
            .saturating_add(16384);
        if bucket_bytes.saturating_add(output_bound) > self.plan.max_bytes.get() {
            return Err(columnar::exhausted());
        }
        drop(self.samples);
        check()?;
        output(&buckets)
    }
}

/* === Public Functions === */

pub fn output_schema(plan: &TimeBucketPlan, input: &Schema) -> Result<Schema> {
    validate(plan, input)?;
    Ok(output(&[])?.schema().as_ref().clone())
}

pub fn apply(plan: &TimeBucketPlan, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
    let mut accumulator = Accumulator::new(plan, input.schema())?;
    accumulator.push(input, check)?;
    accumulator.finish(check)
}

/* === Private Functions === */

fn validate(plan: &TimeBucketPlan, input: &Schema) -> Result<()> {
    if plan.group.len() > crate::arrow::MAX_FIELDS || plan.group.iter().collect::<BTreeSet<_>>().len() != plan.group.len() {
        return Err(columnar::invalid("time bucket groups must be unique bounded fields"));
    }
    let time = field(input, &plan.time)?;
    if !matches!(time.data_type(), DataType::Int64 | DataType::Timestamp(TimeUnit::Nanosecond, _))
        || field(input, &plan.value)?.data_type() != &DataType::Float64
        || field(input, "_storage.position")?.data_type() != &DataType::UInt64
    {
        return Err(columnar::invalid("time buckets require integer time, Float64 value and UInt64 position"));
    }
    field(input, &plan.identity)?;
    for group in &plan.group {
        field(input, group)?;
    }
    Ok(())
}

fn field(schema: &Schema, name: &str) -> Result<Field> {
    if name.is_empty() {
        return Err(columnar::invalid("time bucket field is empty"));
    }
    if let Ok(index) = schema.index_of(name) {
        return Ok(schema.field(index).clone());
    }
    FieldPath::from(name).field(schema)
}
fn array(batch: &RecordBatch, name: &str) -> Result<ArrayRef> {
    if let Some(array) = batch.column_by_name(name) {
        return Ok(array.clone());
    }
    Ok(FieldPath::from(name).array(batch)?.clone())
}
fn time(array: &ArrayRef, row: usize) -> Result<i64> {
    if array.is_null(row) {
        return Err(columnar::invalid("time bucket time is null"));
    }
    if let Some(array) = array.as_any().downcast_ref::<Int64Array>() {
        return Ok(array.value(row));
    }
    if let Some(array) = array.as_any().downcast_ref::<TimestampNanosecondArray>() {
        return Ok(array.value(row));
    }
    Err(columnar::invalid("time bucket time has an incompatible type"))
}
fn scalar_key(array: &ArrayRef, row: usize, budget: usize) -> Result<Vec<u8>> {
    if !array.is_null(row) {
        if let Some(text) = array.as_any().downcast_ref::<StringArray>() {
            let value = text.value(row);
            let length = value.len().saturating_add(value.as_bytes().iter().filter(|byte| **byte == 0).count()).saturating_add(3);
            if length > budget {
                return Err(columnar::exhausted());
            }
            let mut key = Vec::new();
            key.try_reserve_exact(length).map_err(|_| columnar::exhausted())?;
            index::append_text(&mut key, value);
            return Ok(key);
        }
    }
    let value = super::transform::scalar(array.as_ref(), row)?;
    let key = index::encode_key(&[value])?;
    if key.capacity() > budget {
        return Err(columnar::exhausted());
    }
    Ok(key)
}
fn revision(bucket: &Bucket) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(&bucket.key);
    hash.update(bucket.bucket.to_le_bytes());
    hash.update(bucket.count.to_le_bytes());
    for value in [bucket.first, bucket.last, bucket.min, bucket.max, bucket.sum, bucket.sum_squares, bucket.mean, bucket.m2] {
        hash.update(value.to_bits().to_le_bytes());
    }
    for value in [bucket.first_position, bucket.last_position, bucket.min_position, bucket.max_position] {
        hash.update(value.to_le_bytes());
    }
    for position in &bucket.positions {
        hash.update(position.to_le_bytes());
    }
    hash.finalize().into()
}
fn output(buckets: &[Bucket]) -> Result<RecordBatch> {
    let positions =
        ListArray::from_iter_primitive::<UInt64Type, _, _>(buckets.iter().map(|bucket| Some(bucket.positions.iter().copied().map(Some))));
    let columns: Vec<(&str, ArrayRef)> = vec![
        ("key", Arc::new(BinaryArray::from_iter_values(buckets.iter().map(|bucket| bucket.key.as_slice())))),
        ("bucket", Arc::new(Int64Array::from(buckets.iter().map(|bucket| bucket.bucket).collect::<Vec<_>>()))),
        ("revision", Arc::new(BinaryArray::from_iter_values(buckets.iter().map(|bucket| bucket.revision.as_slice())))),
        ("count", Arc::new(UInt64Array::from(buckets.iter().map(|bucket| bucket.count).collect::<Vec<_>>()))),
        ("first", Arc::new(Float64Array::from(buckets.iter().map(|bucket| bucket.first).collect::<Vec<_>>()))),
        ("last", Arc::new(Float64Array::from(buckets.iter().map(|bucket| bucket.last).collect::<Vec<_>>()))),
        ("min", Arc::new(Float64Array::from(buckets.iter().map(|bucket| bucket.min).collect::<Vec<_>>()))),
        ("max", Arc::new(Float64Array::from(buckets.iter().map(|bucket| bucket.max).collect::<Vec<_>>()))),
        ("sum", Arc::new(Float64Array::from(buckets.iter().map(|bucket| bucket.sum).collect::<Vec<_>>()))),
        ("sum_squares", Arc::new(Float64Array::from(buckets.iter().map(|bucket| bucket.sum_squares).collect::<Vec<_>>()))),
        ("mean", Arc::new(Float64Array::from(buckets.iter().map(|bucket| bucket.mean).collect::<Vec<_>>()))),
        ("m2", Arc::new(Float64Array::from(buckets.iter().map(|bucket| bucket.m2).collect::<Vec<_>>()))),
        ("first_position", Arc::new(UInt64Array::from(buckets.iter().map(|bucket| bucket.first_position).collect::<Vec<_>>()))),
        ("last_position", Arc::new(UInt64Array::from(buckets.iter().map(|bucket| bucket.last_position).collect::<Vec<_>>()))),
        ("min_position", Arc::new(UInt64Array::from(buckets.iter().map(|bucket| bucket.min_position).collect::<Vec<_>>()))),
        ("max_position", Arc::new(UInt64Array::from(buckets.iter().map(|bucket| bucket.max_position).collect::<Vec<_>>()))),
        ("positions", Arc::new(positions)),
    ];
    RecordBatch::try_from_iter(columns).map_err(|_| columnar::invalid("invalid time bucket output"))
}

pub(crate) fn fingerprints(batch: &RecordBatch, budget: usize) -> Result<BTreeMap<Vec<u8>, [u8; 32]>> {
    let keys = batch
        .column_by_name("key")
        .and_then(|array| array.as_any().downcast_ref::<BinaryArray>())
        .ok_or_else(|| columnar::invalid("bucket output has no binary keys"))?;
    let revisions = batch
        .column_by_name("revision")
        .and_then(|array| array.as_any().downcast_ref::<BinaryArray>())
        .ok_or_else(|| columnar::invalid("bucket output has no revisions"))?;
    let mut values = BTreeMap::new();
    let mut key_bytes = 0usize;
    for row in 0..batch.num_rows() {
        key_bytes = key_bytes.saturating_add(keys.value(row).len());
        if crate::btree_map_memory::<Vec<u8>, [u8; 32]>(values.len() + 1).saturating_add(key_bytes) > budget {
            return Err(columnar::exhausted());
        }
        let revision = revisions.value(row).try_into().map_err(|_| columnar::invalid("invalid bucket content revision"))?;
        values.insert(keys.value(row).to_vec(), revision);
    }
    Ok(values)
}

/* === Tests === */
