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

pub use harmos_domain::model::storage::query::transform::histogram::Histogram;

/* === Imports === */

use crate::storage::query::transform::FieldPathExecution;
// Standard Library Imports
use std::sync::Arc;
// Third Party Imports
use arrow_array::{Float64Array, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
// Local Imports
use super::downsample::number;
use super::{invalid, numeric_type};
use crate::Result;
use harmos_domain::model::storage::query::scalar::ScalarValue;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Bins {
    plan: Histogram,
    schema: Arc<Schema>,
    counts: Vec<u64>,
}

/* === Enums === */

/* === Traits === */

pub trait HistogramExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn start(&self, input: &Schema) -> Result<Bins>;

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch>;
}

/* === Implementations === */

impl HistogramExecution for Histogram {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.edges.len() < 2
            || self.edges.len() > 65536
            || self.edges.iter().any(|edge| !edge.is_finite())
            || self.edges.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid("histogram requires 2..65536 finite strictly increasing edges"));
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

    fn start(&self, input: &Schema) -> Result<Bins> {
        Ok(Bins {
            schema: Arc::new(self.output_schema(input)?),
            plan: self.clone(),
            counts: vec![0; self.edges.len() + 1],
        })
    }

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        let mut bins = self.start(input.schema().as_ref())?;
        bins.push(input, check)?;
        bins.finish()
    }
}

impl Bins {
    pub fn push(&mut self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
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
            counts[bin] = counts[bin].checked_add(1).ok_or_else(|| invalid("histogram count overflow"))?;
        }
        self.counts = counts;
        Ok(())
    }

    pub fn finish(self) -> Result<RecordBatch> {
        let lower = std::iter::once(None).chain(self.plan.edges.iter().copied().map(Some)).collect::<Vec<_>>();
        let upper = self.plan.edges.iter().copied().map(Some).chain(std::iter::once(None)).collect::<Vec<_>>();
        RecordBatch::try_new(
            self.schema,
            vec![Arc::new(Float64Array::from(lower)), Arc::new(Float64Array::from(upper)), Arc::new(UInt64Array::from(self.counts))],
        )
        .map_err(|_| invalid("histogram output batch is invalid"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
