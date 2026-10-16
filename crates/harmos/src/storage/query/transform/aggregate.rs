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

/* === Imports === */
// Standard Library Imports
use std::cmp::Ordering;
use std::sync::Arc;
// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Field, Schema};
// Local Imports
use super::project::Projection;
use super::{FieldPath, exact_type, invalid, numeric_type, scalar_type, values_array};
use crate::storage::codec::{Nested, Repeated, Text, product};
use crate::storage::reduce::{Accumulator, Agg, ScalarValue, Scratch, sort_values};
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct Aggregate {
    pub groups: Vec<Projection>,
    pub measures: Vec<Measure>,
}
#[derive(Clone, Debug)]
pub struct Measure {
    pub field: FieldPath,
    pub statistic: crate::Aggregate,
    pub name: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Aggregate {
    pub fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.groups.len() > 64 || self.measures.is_empty() || self.measures.len() > 64 {
            return Err(invalid(
                "aggregate requires at most 64 groups and 1..64 measures",
            ));
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
            if !matches!(
                statistic,
                crate::Aggregate::Count | crate::Aggregate::Min | crate::Aggregate::Max
            ) && !numeric_type(field.data_type())
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
            let mut field = Field::new(
                &measure.name,
                output,
                !matches!(statistic, crate::Aggregate::Count),
            );
            if field.data_type() == &exact_type() {
                field = field.with_metadata(std::collections::HashMap::from([(
                    "harmos.logical_type".to_owned(),
                    "exact-rational-v1".to_owned(),
                )]));
            }
            fields.push(field);
        }
        for field in &fields {
            if field.name().is_empty()
                || fields
                    .iter()
                    .filter(|other| other.name() == field.name())
                    .count()
                    != 1
            {
                return Err(invalid(
                    "aggregate output names must be nonempty and unique",
                ));
            }
        }
        Ok(Schema::new(fields))
    }

    pub fn apply(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<RecordBatch> {
        let schema = Arc::new(self.output_schema(input.schema().as_ref())?);
        let mut order = (0..input.num_rows()).collect::<Vec<_>>();
        sort_values(&mut order, |left, right| {
            check()?;
            let comparison = self.compare_groups(input, left, right)?;
            Ok(comparison.then(left.cmp(&right)))
        })?;
        let mut output = vec![Vec::new(); schema.fields().len()];
        let mut start = 0usize;
        // An ungrouped empty input still yields the aggregate identity row.
        while start < order.len() || (order.is_empty() && self.groups.is_empty() && start == 0) {
            check()?;
            let mut end = start;
            while end < order.len()
                && (end == start
                    || self
                        .compare_groups(input, order[start], order[end])?
                        .is_eq())
            {
                end += 1;
            }
            for (index, group) in self.groups.iter().enumerate() {
                output[index].push(normalized(group.field.value(input, order[start])?));
            }
            for (index, measure) in self.measures.iter().enumerate() {
                let mut accumulator = Accumulator::new(Agg::Of(measure.statistic));
                let slots = (end - start)
                    .checked_mul(2)
                    .and_then(|slots| slots.checked_add(4))
                    .ok_or_else(|| invalid("aggregate scratch size overflow"))?;
                let mut scratch = Scratch::new(slots);
                for row in &order[start..end] {
                    check()?;
                    let value = normalized(measure.field.value(input, *row)?);
                    accumulator.push(*row as u64, &value, &mut scratch)?;
                }
                output[self.groups.len() + index].push(accumulator.finish(check)?);
            }
            if end == order.len() {
                break;
            }
            start = end;
        }
        let mut columns = Vec::with_capacity(output.len());
        for (field, values) in schema.fields().iter().zip(output) {
            columns.push(values_array(field.data_type(), &values)?);
        }
        RecordBatch::try_new(schema, columns)
            .map_err(|_| invalid("aggregate output batch is invalid"))
    }

    fn compare_groups(&self, input: &RecordBatch, left: usize, right: usize) -> Result<Ordering> {
        for group in &self.groups {
            let left = normalized(group.field.value(input, left)?);
            let right = normalized(group.field.value(input, right)?);
            let comparison = left.compare(&right)?;
            if !comparison.is_eq() {
                return Ok(comparison);
            }
        }
        Ok(Ordering::Equal)
    }
}
product!(Aggregate { groups: Repeated<Projection> => 1, measures: Repeated<Measure> => 2 });
product!(Measure { field: Nested<FieldPath> => 1, statistic: Nested<crate::Aggregate> => 2, name: Text => 3 });
impl MemorySize for Aggregate {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .groups
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.groups))
            + self
                .measures
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.measures))
    }
}
impl MemorySize for Measure {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .field
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.field))
            + self.name.capacity()
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn normalized(value: ScalarValue) -> ScalarValue {
    match value {
        ScalarValue::Float(number) if !number.is_finite() => ScalarValue::Null,
        value => value,
    }
}

/* === Tests === */
