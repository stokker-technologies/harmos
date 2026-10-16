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
use std::sync::Arc;
// Third Party Imports
use arrow_array::{Float64Array, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
// Local Imports
use super::downsample::number;
use super::{FieldPath, invalid, numeric_type};
use crate::storage::codec::{Float, Many, Nested, product};
use crate::storage::query::scalar::ScalarValue;
use crate::{MemorySize, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct Histogram {
    pub field: FieldPath,
    pub edges: Vec<f64>,
}

/// Incremental histogram state.
pub struct Bins {
    plan: Histogram,
    schema: Arc<Schema>,
    counts: Vec<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Histogram {
    pub fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.edges.len() < 2
            || self.edges.len() > 65536
            || self.edges.iter().any(|edge| !edge.is_finite())
            || self.edges.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                "histogram requires 2..65536 finite strictly increasing edges",
            ));
        }
        if !numeric_type(self.field.field(input)?.data_type()) {
            return Err(invalid("histogram requires numeric field"));
        }
        Ok(Schema::new(vec![
            Field::new("lower", DataType::Float64, true),
            Field::new("upper", DataType::Float64, true),
            Field::new("count", DataType::UInt64, false),
        ]))
    }

    /// Start incremental binning over batches of `input`.
    pub fn start(&self, input: &Schema) -> Result<Bins> {
        Ok(Bins {
            schema: Arc::new(self.output_schema(input)?),
            plan: self.clone(),
            counts: vec![0; self.edges.len() + 1],
        })
    }

    pub fn apply(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<RecordBatch> {
        let mut bins = self.start(input.schema().as_ref())?;
        bins.push(input, check)?;
        bins.finish()
    }
}

impl Bins {
    /// Bytes retained by bin counts between batches.
    pub fn retained_bytes(&self) -> usize {
        self.counts.len().saturating_mul(16).saturating_add(1024)
    }

    /// Bytes needed to materialize the final output batch.
    pub fn finish_bytes(&self) -> usize {
        self.counts.len().saturating_mul(96).saturating_add(4096)
    }

    /// Count one batch. First and last bins hold finite values outside the
    /// edges; a failed batch leaves the counts unchanged.
    pub fn push(
        &mut self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        let edges = &self.plan.edges;
        let mut counts = self.counts.clone();
        for row in 0..input.num_rows() {
            check()?;
            let value = self.plan.field.value(input, row)?;
            if matches!(value, ScalarValue::Null | ScalarValue::Missing) {
                continue;
            }
            let value = number(&value)?;
            if !value.is_finite() {
                continue;
            }
            let bin = if value < edges[0] {
                0
            } else if value > edges[edges.len() - 1] {
                edges.len()
            } else if value == edges[edges.len() - 1] {
                edges.len() - 1
            } else {
                edges.partition_point(|edge| *edge <= value)
            };
            counts[bin] = counts[bin]
                .checked_add(1)
                .ok_or_else(|| invalid("histogram count overflow"))?;
        }
        self.counts = counts;
        Ok(())
    }

    pub fn finish(self) -> Result<RecordBatch> {
        let lower = std::iter::once(None)
            .chain(self.plan.edges.iter().copied().map(Some))
            .collect::<Vec<_>>();
        let upper = self
            .plan
            .edges
            .iter()
            .copied()
            .map(Some)
            .chain(std::iter::once(None))
            .collect::<Vec<_>>();
        RecordBatch::try_new(
            self.schema,
            vec![
                Arc::new(Float64Array::from(lower)),
                Arc::new(Float64Array::from(upper)),
                Arc::new(UInt64Array::from(self.counts)),
            ],
        )
        .map_err(|_| invalid("histogram output batch is invalid"))
    }
}
product!(Histogram { field: Nested<FieldPath> => 1, edges: Many<Float> => 2 });
impl MemorySize for Histogram {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .field
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.field))
            + self.edges.capacity().saturating_mul(8)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
