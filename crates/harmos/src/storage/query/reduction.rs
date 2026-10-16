/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Incremental Arrow query reductions

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::cmp::Ordering;
use std::sync::Arc;
// Third Party Imports
use arrow_array::{Float64Array, RecordBatch, UInt64Array};
use arrow_schema::Schema;
// Local Imports
use super::execute::{Pipeline, working_bound};
use super::transform::aggregate::Aggregate;
use super::transform::downsample::number;
use super::transform::histogram::Histogram;
use super::transform::{Transform, invalid, values_array};
use crate::storage::arrow::{ArrowBatch, RetainedMemory};
use crate::storage::reduce::{Accumulator, Agg, ScalarValue, Scratch, sort_values};
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
/// A finite query whose first global transform reduces batches incrementally.
pub struct Reduction {
    before: Pipeline,
    after: Pipeline,
    state: State,
    capture: u64,
    retained: Vec<RetainedMemory>,
}

struct Group {
    keys: Vec<ScalarValue>,
    accumulators: Vec<Accumulator>,
}

/* === Enums === */
enum State {
    Aggregate {
        plan: Aggregate,
        schema: Arc<Schema>,
        groups: Vec<Group>,
    },
    Histogram {
        plan: Histogram,
        schema: Arc<Schema>,
        counts: Vec<u64>,
    },
}

/* === Traits === */

/* === Implementations === */
impl Reduction {
    /// Retained Plan Descriptors Only; Group Data And Counts Have Separate Owners.
    pub fn memory_size(&self) -> usize {
        let state = match &self.state {
            State::Aggregate { plan, schema, .. } => plan
                .memory_size()
                .saturating_add(crate::storage::arrow::schema_memory(schema)),
            State::Histogram { plan, schema, .. } => plan
                .memory_size()
                .saturating_add(crate::storage::arrow::schema_memory(schema)),
        };
        std::mem::size_of::<Self>()
            .saturating_add(self.before.memory_size())
            .saturating_add(self.after.memory_size())
            .saturating_add(state)
    }

    pub fn working_memory(&self, input: &RecordBatch) -> Result<usize> {
        let (prefix, rows, resident) = self.before.native_memory(input)?;
        let schema = self.before.output_schema();
        let (width, output_schema) = match &self.state {
            State::Aggregate {
                plan,
                schema: output,
                ..
            } => (plan.groups.len() + plan.measures.len(), output),
            State::Histogram { schema: output, .. } => (schema.fields().len(), output),
        };
        let scratch = resident
            .checked_mul(8)
            .and_then(|bytes| {
                rows.max(1)
                    .checked_mul(width.max(schema.fields().len()).max(1))
                    .and_then(|cells| cells.checked_mul(512))
                    .and_then(|scratch| bytes.checked_add(scratch))
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    crate::storage::arrow::schema_memory(output_schema).saturating_mul(4),
                )
            })
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| invalid("incremental scratch estimate overflow"))?;
        let retained = match &self.state {
            State::Aggregate { plan, .. } => {
                let groups = rows.max(usize::from(plan.groups.is_empty()));
                let fixed = plan
                    .groups
                    .len()
                    .checked_mul(std::mem::size_of::<ScalarValue>())
                    .and_then(|bytes| {
                        plan.measures
                            .len()
                            .checked_mul(std::mem::size_of::<Accumulator>() * 4)
                            .and_then(|accumulators| bytes.checked_add(accumulators))
                    })
                    .and_then(|bytes| bytes.checked_mul(4))
                    .and_then(|bytes| bytes.checked_add(1024))
                    .and_then(|bytes| groups.checked_mul(bytes))
                    .ok_or_else(|| invalid("aggregate group estimate overflow"))?;
                let key_text = resident
                    .checked_mul(plan.groups.len())
                    .and_then(|bytes| bytes.checked_mul(4))
                    .ok_or_else(|| invalid("aggregate key estimate overflow"))?;
                let retained_measures = plan
                    .measures
                    .iter()
                    .filter(|measure| {
                        Agg::Of(measure.statistic).retains_values()
                            || measure.field.field(&schema).is_ok_and(|field| {
                                field.data_type() == &arrow_schema::DataType::Utf8
                            })
                    })
                    .count();
                let cells = rows
                    .checked_mul(retained_measures)
                    .and_then(|cells| {
                        cells.checked_mul(std::mem::size_of::<ScalarValue>() * 4 + 128)
                    })
                    .ok_or_else(|| invalid("aggregate value estimate overflow"))?;
                let text = resident
                    .checked_mul(retained_measures)
                    .and_then(|bytes| bytes.checked_mul(4))
                    .ok_or_else(|| invalid("aggregate text estimate overflow"))?;
                fixed
                    .checked_add(key_text)
                    .and_then(|bytes| bytes.checked_add(cells))
                    .and_then(|bytes| bytes.checked_add(text))
                    .ok_or_else(|| invalid("aggregate retained estimate overflow"))?
            }
            State::Histogram { plan, .. } => (plan.edges.len() + 1)
                .checked_mul(16)
                .and_then(|bytes| bytes.checked_add(1024))
                .ok_or_else(|| invalid("histogram estimate overflow"))?,
        };
        prefix
            .checked_add(scratch)
            .and_then(|bytes| bytes.checked_add(retained))
            .ok_or_else(|| invalid("incremental query memory estimate overflow"))
    }

    pub fn finish_memory(&self) -> Result<usize> {
        let (rows, output_bytes) = match &self.state {
            State::Aggregate {
                plan,
                schema,
                groups,
            } => {
                let count = groups.len().max(usize::from(plan.groups.is_empty()));
                let text = groups
                    .iter()
                    .flat_map(|group| &group.keys)
                    .map(MemorySize::memory_size)
                    .try_fold(0usize, |total, bytes| total.checked_add(bytes))
                    .ok_or_else(|| invalid("aggregate output estimate overflow"))?;
                let bytes = count
                    .checked_mul(schema.fields().len())
                    .and_then(|cells| cells.checked_mul(512))
                    .and_then(|bytes| bytes.checked_add(text.saturating_mul(4)))
                    .and_then(|bytes| bytes.checked_add(65536))
                    .and_then(|bytes| {
                        if groups.is_empty() && plan.groups.is_empty() {
                            group_bound(plan, &[])
                                .ok()
                                .and_then(|group| bytes.checked_add(group))
                        } else {
                            Some(bytes)
                        }
                    })
                    .ok_or_else(|| invalid("aggregate finish estimate overflow"))?;
                (count, bytes)
            }
            State::Histogram { plan, .. } => {
                let count = plan.edges.len() + 1;
                let bytes = count
                    .checked_mul(96)
                    .and_then(|bytes| bytes.checked_add(4096))
                    .ok_or_else(|| invalid("histogram finish estimate overflow"))?;
                (count, bytes)
            }
        };
        output_bytes
            .checked_add(self.after.complete_shape_memory(rows, output_bytes)?)
            .ok_or_else(|| invalid("incremental finish memory estimate overflow"))
    }

    pub(super) fn new(before: Pipeline, transform: Transform, after: Pipeline) -> Result<Self> {
        let schema = Arc::new(transform.output_schema(before.output_schema().as_ref())?);
        let state = match transform {
            Transform::Aggregate(plan) => State::Aggregate {
                plan,
                schema,
                groups: Vec::new(),
            },
            Transform::Histogram(plan) => {
                let counts = Vec::new();
                State::Histogram {
                    plan,
                    schema,
                    counts,
                }
            }
            _ => {
                return Err(invalid(
                    "incremental execution requires aggregate or histogram",
                ));
            }
        };
        Ok(Self {
            before,
            after,
            state,
            capture: 0,
            retained: Vec::new(),
        })
    }

    pub fn push(
        &mut self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
        reserve: &mut impl FnMut(usize) -> Result<RetainedMemory>,
    ) -> Result<()> {
        self.before.check_input(input)?;
        let input = self.before.apply(input.clone(), check, reserve)?;
        let input = &input;
        // Scalar extraction and group lookup may temporarily clone text keys.
        let transient = match &self.state {
            State::Aggregate { plan, .. } => Transform::Aggregate(plan.clone()),
            State::Histogram { plan, .. } => Transform::Histogram(plan.clone()),
        };
        let _scratch = reserve(working_bound(input, &transient)?)?;
        match &mut self.state {
            State::Aggregate { plan, groups, .. } => {
                if plan.groups.is_empty() && groups.is_empty() {
                    let memory = reserve(group_bound(plan, &[])?)?;
                    groups.push(new_group(plan, Vec::new()));
                    self.retained.push(memory);
                }
                for row in 0..input.num_rows() {
                    check()?;
                    let keys = plan
                        .groups
                        .iter()
                        .map(|group| group.field.value(input, row).map(normalized))
                        .collect::<Result<Vec<_>>>()?;
                    let mut found = None;
                    for (index, group) in groups.iter().enumerate() {
                        if compare_keys(&group.keys, &keys)?.is_eq() {
                            found = Some(index);
                            break;
                        }
                    }
                    let index = match found {
                        Some(index) => index,
                        None => {
                            let memory = reserve(group_bound(plan, &keys)?)?;
                            groups.push(new_group(plan, keys));
                            self.retained.push(memory);
                            groups.len() - 1
                        }
                    };
                    for (measure, accumulator) in
                        plan.measures.iter().zip(&mut groups[index].accumulators)
                    {
                        let value = normalized(measure.field.value(input, row)?);
                        // Retained order statistics grow only by selected scalars,
                        // rather than retaining the original source RecordBatch.
                        let retains = Agg::Of(measure.statistic).retains_values();
                        if retains || matches!(value, ScalarValue::Text(_)) {
                            let bytes = value
                                .memory_size()
                                .checked_mul(4)
                                .and_then(|bytes| bytes.checked_add(128))
                                .ok_or_else(|| invalid("aggregate retained memory overflow"))?;
                            self.retained.push(reserve(bytes)?);
                        }
                        accumulator.push(self.capture, &value, &mut Scratch::new(usize::MAX))?;
                    }
                    self.capture = self
                        .capture
                        .checked_add(1)
                        .ok_or_else(|| invalid("query capture count overflow"))?;
                }
            }
            State::Histogram { plan, counts, .. } => {
                if counts.is_empty() {
                    self.retained.push(reserve(
                        (plan.edges.len() + 1)
                            .checked_mul(16)
                            .and_then(|bytes| bytes.checked_add(1024))
                            .ok_or_else(|| invalid("histogram memory overflow"))?,
                    )?);
                    counts.resize(plan.edges.len() + 1, 0);
                }
                for row in 0..input.num_rows() {
                    check()?;
                    let value = normalized(plan.field.value(input, row)?);
                    if matches!(value, ScalarValue::Null | ScalarValue::Missing) {
                        continue;
                    }
                    let value = number(&value)?;
                    if !value.is_finite() {
                        continue;
                    }
                    let index = if value < plan.edges[0] {
                        0
                    } else if value > plan.edges[plan.edges.len() - 1] {
                        plan.edges.len()
                    } else if value == plan.edges[plan.edges.len() - 1] {
                        plan.edges.len() - 1
                    } else {
                        plan.edges.partition_point(|edge| *edge <= value)
                    };
                    counts[index] = counts[index]
                        .checked_add(1)
                        .ok_or_else(|| invalid("histogram count overflow"))?;
                }
            }
        }
        Ok(())
    }

    pub fn finish(
        mut self,
        check: &mut impl FnMut() -> Result<()>,
        reserve: &mut impl FnMut(usize) -> Result<RetainedMemory>,
    ) -> Result<ArrowBatch> {
        check()?;
        let input = match self.state {
            State::Aggregate {
                plan,
                schema,
                mut groups,
            } => {
                if groups.is_empty() && plan.groups.is_empty() {
                    self.retained.push(reserve(group_bound(&plan, &[])?)?);
                    groups.push(new_group(&plan, Vec::new()));
                }
                let count = groups.len();
                let scalar_bytes = groups
                    .iter()
                    .flat_map(|group| &group.keys)
                    .map(MemorySize::memory_size)
                    .try_fold(0usize, |sum, size| {
                        sum.checked_add(size)
                            .ok_or_else(|| invalid("aggregate output memory overflow"))
                    })?;
                let output_bytes = count
                    .checked_mul(schema.fields().len())
                    .and_then(|cells| cells.checked_mul(512))
                    .and_then(|bytes| bytes.checked_add(scalar_bytes.saturating_mul(4)))
                    .and_then(|bytes| bytes.checked_add(65536))
                    .ok_or_else(|| invalid("aggregate output memory overflow"))?;
                let output_memory = reserve(output_bytes)?;
                let mut order = (0..count).collect::<Vec<_>>();
                sort_values(&mut order, |left, right| {
                    check()?;
                    compare_keys(&groups[left].keys, &groups[right].keys)
                })?;
                let mut values = vec![Vec::with_capacity(count); schema.fields().len()];
                // Move accumulators out by index without cloning retained scratch.
                let mut groups = groups.into_iter().map(Some).collect::<Vec<_>>();
                for index in order {
                    check()?;
                    let group = groups[index].take().expect("unique sorted group index");
                    for (column, key) in group.keys.into_iter().enumerate() {
                        values[column].push(key);
                    }
                    for (column, accumulator) in group.accumulators.into_iter().enumerate() {
                        values[plan.groups.len() + column].push(accumulator.finish(check)?);
                    }
                }
                let columns = schema
                    .fields()
                    .iter()
                    .zip(values)
                    .map(|(field, values)| values_array(field.data_type(), &values))
                    .collect::<Result<Vec<_>>>()?;
                let output = RecordBatch::try_new(schema, columns)
                    .map_err(|_| invalid("aggregate output batch is invalid"))?;
                output_memory
                    .shrink_to(crate::storage::arrow::new_resident_bytes(&output, &[])?)?;
                crate::storage::arrow::retain_native(output, output_memory)?
            }
            State::Histogram {
                plan,
                schema,
                mut counts,
            } => {
                let memory = reserve(
                    (plan.edges.len() + 1)
                        .checked_mul(96)
                        .and_then(|bytes| bytes.checked_add(4096))
                        .ok_or_else(|| invalid("histogram output memory overflow"))?,
                )?;
                if counts.is_empty() {
                    counts.resize(plan.edges.len() + 1, 0);
                }
                let lower = std::iter::once(None)
                    .chain(plan.edges.iter().copied().map(Some))
                    .collect::<Vec<_>>();
                let upper = plan
                    .edges
                    .iter()
                    .copied()
                    .map(Some)
                    .chain(std::iter::once(None))
                    .collect::<Vec<_>>();
                let output = RecordBatch::try_new(
                    schema,
                    vec![
                        Arc::new(Float64Array::from(lower)),
                        Arc::new(Float64Array::from(upper)),
                        Arc::new(UInt64Array::from(counts)),
                    ],
                )
                .map_err(|_| invalid("histogram output batch is invalid"))?;
                memory.shrink_to(crate::storage::arrow::new_resident_bytes(&output, &[])?)?;
                crate::storage::arrow::retain_native(output, memory)?
            }
        };
        // The post-reduction pipeline may itself materialize a small derived set.
        let output = self.after.execute(&[input], check, reserve)?;
        drop(self.retained);
        Ok(output)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn group_bound(plan: &Aggregate, keys: &[ScalarValue]) -> Result<usize> {
    keys.iter()
        .map(MemorySize::memory_size)
        .try_fold(0usize, |total, bytes| total.checked_add(bytes))
        .and_then(|bytes| {
            plan.measures
                .len()
                .checked_mul(std::mem::size_of::<Accumulator>() * 4)
                .and_then(|accumulators| bytes.checked_add(accumulators))
        })
        .and_then(|bytes| bytes.checked_mul(4))
        .and_then(|bytes| bytes.checked_add(1024))
        .ok_or_else(|| invalid("aggregate group memory overflow"))
}

fn new_group(plan: &Aggregate, keys: Vec<ScalarValue>) -> Group {
    Group {
        keys,
        accumulators: plan
            .measures
            .iter()
            .map(|measure| Accumulator::new(Agg::Of(measure.statistic)))
            .collect(),
    }
}

fn compare_keys(left: &[ScalarValue], right: &[ScalarValue]) -> Result<Ordering> {
    for (left, right) in left.iter().zip(right) {
        let comparison = left.compare(right)?;
        if !comparison.is_eq() {
            return Ok(comparison);
        }
    }
    Ok(left.len().cmp(&right.len()))
}

fn normalized(value: ScalarValue) -> ScalarValue {
    match value {
        ScalarValue::Float(number) if !number.is_finite() => ScalarValue::Null,
        value => value,
    }
}

/* === Tests === */
