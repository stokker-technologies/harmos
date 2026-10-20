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

pub use harmos_domain::model::storage::query::transform::aggregate::{Aggregate, Measure};

/* === Imports === */

use crate::storage::query::transform::FieldPathExecution;
// Standard Library Imports
use std::cmp::Ordering;
use std::sync::Arc;
// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Field, Schema};
// Local Imports
use super::{exact_type, invalid, numeric_type, scalar_type, values_array};
use crate::storage::query::accumulate::{Accumulator, Scratch};
use crate::{MemorySize, Result};
use harmos_domain::model::storage::query::scalar::{Agg, ScalarValue};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Groups {
    plan: Aggregate,
    schema: Arc<Schema>,
    groups: Vec<Group>,
    capture: u64,
    retained: usize,
}

struct Group {
    keys: Vec<ScalarValue>,
    accumulators: Vec<Accumulator>,
}

/* === Enums === */

/* === Traits === */

pub trait AggregateExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn start(&self, input: &Schema) -> Result<Groups>;

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch>;
}

/* === Implementations === */

impl AggregateExecution for Aggregate {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.groups.len() > 64 || self.measures.is_empty() || self.measures.len() > 64 {
            return Err(invalid("aggregate requires at most 64 groups and 1..64 measures"));
        }
        let mut fields = Vec::new();
        for group in &self.groups {
            let field = group.field.field(input)?;
            if !scalar_type(field.data_type()) {
                return Err(invalid("aggregate grouping requires scalar keys"));
            }
            fields.push(field.with_name(&group.name).with_nullable(true));
        }
        for measure in &self.measures {
            let field = measure.field.field(input)?;
            if !scalar_type(field.data_type()) {
                return Err(invalid("aggregate requires scalar values"));
            }
            let statistic = measure.statistic;
            if !matches!(statistic, crate::Aggregate::Count | crate::Aggregate::Min | crate::Aggregate::Max)
                && !numeric_type(field.data_type())
            {
                return Err(invalid("numeric aggregate requires numeric field"));
            }
            if let crate::Aggregate::Percentile(rank) = statistic {
                // The shared vocabulary validates the rank through its normal codec.
                if rank > 100 {
                    return Err(invalid("invalid percentile rank"));
                }
            }
            let output = match statistic {
                crate::Aggregate::Count => DataType::UInt64,
                crate::Aggregate::Min | crate::Aggregate::Max => field.data_type().clone(),
                crate::Aggregate::Sum | crate::Aggregate::Mean | crate::Aggregate::Median
                    if !matches!(field.data_type(), DataType::Float32 | DataType::Float64) =>
                {
                    exact_type()
                }
                _ => DataType::Float64,
            };
            let mut field = Field::new(&measure.name, output, !matches!(statistic, crate::Aggregate::Count));
            if field.data_type() == &exact_type() {
                field = field
                    .with_metadata(std::collections::HashMap::from([("harmos.logical_type".to_owned(), "exact-rational-v1".to_owned())]));
            }
            fields.push(field);
        }
        for field in &fields {
            if field.name().is_empty() || fields.iter().filter(|other| other.name() == field.name()).count() != 1 {
                return Err(invalid("aggregate output names must be nonempty and unique"));
            }
        }
        Ok(Schema::new(fields))
    }

    fn start(&self, input: &Schema) -> Result<Groups> {
        let schema = Arc::new(self.output_schema(input)?);
        let mut groups = Groups { plan: self.clone(), schema, groups: Vec::new(), capture: 0, retained: 0 };
        // An ungrouped reduction always yields its identity row, even for no input.
        if self.groups.is_empty() {
            groups.retained = group_bound(self, &[])?;
            groups.groups.push(new_group(self, Vec::new()));
        }
        Ok(groups)
    }

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        let mut groups = self.start(input.schema().as_ref())?;
        groups.push(input, check)?;
        groups.finish(check)
    }
}

impl Groups {
    pub fn retained_bytes(&self) -> usize {
        self.retained
    }

    pub fn finish_bytes(&self) -> Result<usize> {
        let text = self
            .groups
            .iter()
            .flat_map(|group| &group.keys)
            .map(MemorySize::memory_size)
            .try_fold(0usize, |total, bytes| total.checked_add(bytes))
            .ok_or_else(|| invalid("aggregate output estimate overflow"))?;
        self.groups
            .len()
            .max(1)
            .checked_mul(self.schema.fields().len())
            .and_then(|cells| cells.checked_mul(512))
            .and_then(|bytes| bytes.checked_add(text.saturating_mul(4)))
            .and_then(|bytes| bytes.checked_add(65536))
            .ok_or_else(|| invalid("aggregate finish estimate overflow"))
    }

    pub fn push(&mut self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        // Extract Keys And Values Before Any Group Changes.
        let mut rows = Vec::with_capacity(input.num_rows());
        for row in 0..input.num_rows() {
            check()?;
            let keys = self.plan.groups.iter().map(|group| group.field.value(input, row).map(normalized)).collect::<Result<Vec<_>>>()?;
            let values =
                self.plan.measures.iter().map(|measure| measure.field.value(input, row).map(normalized)).collect::<Result<Vec<_>>>()?;
            rows.push((keys, values));
        }

        // Fold Rows Into Sorted Groups.
        for (keys, values) in rows {
            check()?;
            let index = match self.find(&keys)? {
                Ok(index) => index,
                Err(index) => {
                    let bytes = group_bound(&self.plan, &keys)?;
                    self.groups.insert(index, new_group(&self.plan, keys));
                    self.retained = self.retained.saturating_add(bytes);
                    index
                }
            };
            let group = &mut self.groups[index];
            for ((measure, accumulator), value) in self.plan.measures.iter().zip(&mut group.accumulators).zip(values) {
                // Retained order statistics grow only by the selected scalars.
                if Agg::Of(measure.statistic).retains_values() || matches!(value, ScalarValue::Text(_)) {
                    let bytes = value.memory_size().saturating_mul(4).saturating_add(128);
                    self.retained = self.retained.saturating_add(bytes);
                }
                accumulator.push(self.capture, &value, &mut Scratch::new(usize::MAX))?;
            }
            self.capture = self.capture.checked_add(1).ok_or_else(|| invalid("query capture count overflow"))?;
        }
        Ok(())
    }

    pub fn finish(self, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        check()?;
        let width = self.schema.fields().len();
        let mut values = vec![Vec::with_capacity(self.groups.len()); width];
        for group in self.groups {
            check()?;
            for (column, key) in group.keys.into_iter().enumerate() {
                values[column].push(key);
            }
            for (column, accumulator) in group.accumulators.into_iter().enumerate() {
                values[self.plan.groups.len() + column].push(accumulator.finish(check)?);
            }
        }
        let columns = self
            .schema
            .fields()
            .iter()
            .zip(values)
            .map(|(field, values)| values_array(field.data_type(), &values))
            .collect::<Result<Vec<_>>>()?;
        RecordBatch::try_new(self.schema, columns).map_err(|_| invalid("aggregate output batch is invalid"))
    }

    // Binary Search Over Groups Kept In Key Order.
    fn find(&self, keys: &[ScalarValue]) -> Result<std::result::Result<usize, usize>> {
        let (mut low, mut high) = (0usize, self.groups.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match compare_keys(&self.groups[middle].keys, keys)? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(low))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn group_bound(plan: &Aggregate, keys: &[ScalarValue]) -> Result<usize> {
    keys.iter()
        .map(MemorySize::memory_size)
        .try_fold(0usize, |total, bytes| total.checked_add(bytes))
        .and_then(|bytes| {
            plan.measures.len().checked_mul(std::mem::size_of::<Accumulator>() * 4).and_then(|accumulators| bytes.checked_add(accumulators))
        })
        .and_then(|bytes| bytes.checked_mul(4))
        .and_then(|bytes| bytes.checked_add(1024))
        .ok_or_else(|| invalid("aggregate group memory overflow"))
}

fn new_group(plan: &Aggregate, keys: Vec<ScalarValue>) -> Group {
    Group { keys, accumulators: plan.measures.iter().map(|measure| Accumulator::new(Agg::Of(measure.statistic))).collect() }
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
