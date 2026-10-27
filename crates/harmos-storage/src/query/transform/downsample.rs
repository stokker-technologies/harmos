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

pub use harmos_domain::model::storage::query::transform::downsample::{Algorithm, Downsample};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Schema, TimeUnit};
use harmos_domain::model::storage::query::scalar::ScalarValue;

// Local Imports
use super::{invalid, numeric_type};
use crate::Result;
use crate::storage::query::transform::FieldPathExecution;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait DownsampleExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch>;
}

/* === Implementations === */

impl DownsampleExecution for Downsample {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.max_rows == 0 {
            return Err(invalid("downsample max_rows must be positive"));
        }
        if !matches!(self.algorithm, Algorithm::Decimate) {
            let field = self.value.as_ref().ok_or_else(|| invalid("downsampling algorithm requires value field"))?.field(input)?;
            if !numeric_type(field.data_type()) {
                return Err(invalid("downsampling requires numeric values"));
            }
            let minimum = if matches!(self.algorithm, Algorithm::Extrema) { 2 } else { 3 };
            if self.max_rows < minimum {
                return Err(invalid("downsampling output bound is below algorithm minimum"));
            }
        }
        if matches!(self.algorithm, Algorithm::LttbLog) && self.axis.is_none() {
            return Err(invalid("logarithmic LTTB requires an explicit positive numeric axis"));
        }
        if let Some(axis) = &self.axis {
            let field = axis.field(input)?;
            if matches!(self.algorithm, Algorithm::LttbLog) && !numeric_type(field.data_type()) {
                return Err(invalid("logarithmic LTTB requires a numeric axis"));
            }
            if !numeric_type(field.data_type()) && !matches!(field.data_type(), DataType::Timestamp(TimeUnit::Nanosecond, _)) {
                return Err(invalid("downsample axis must be numeric or a timestamp"));
            }
        }
        Ok(input.clone())
    }

    fn apply(&self, input: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<RecordBatch> {
        if input.num_rows() <= self.max_rows {
            return Ok(input.clone());
        }
        let mut spool = crate::query::spill::Builder::new(input.schema())?;
        spool.push(input, check)?;
        let output = crate::query::reduction::downsample(spool.finish()?, self, check)?;
        let mut pages = output.pages()?;
        let mut batches = Vec::new();
        while let Some(batch) = pages.next()? {
            check()?;
            batches.push(batch);
        }
        // This Eager API Owns Only Its Requested Output, Never Coordinate Copies.
        arrow_select::concat::concat_batches(&input.schema(), batches.iter()).map_err(|_| invalid("downsample output construction failed"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(in crate::query) fn number(value: &ScalarValue) -> Result<f64> {
    match value {
        ScalarValue::Float(value) => Ok(*value),
        value => {
            let exact = harmos_domain::model::storage::query::scalar::exact(value)
                .ok_or_else(|| invalid("numeric field has a null or nonnumeric value"))?;
            let value = exact.numerator as f64 / exact.denominator.get() as f64;
            Ok(if exact.negative { -value } else { value })
        }
    }
}

/* === Tests === */
