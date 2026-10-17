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
use crate::storage::query::transform::FieldPathExecution;
// Third Party Imports
use arrow_array::RecordBatch;
use arrow_schema::{DataType, Schema, TimeUnit};
// Local Imports
use super::{invalid, numeric_type, take};
use crate::Result;
use harmos_domain::model::storage::query::scalar::ScalarValue;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */
pub trait DownsampleExecution {
    fn output_schema(&self, input: &Schema) -> Result<Schema>;

    fn apply(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<RecordBatch>;
}

/* === Implementations === */
impl DownsampleExecution for Downsample {
    fn output_schema(&self, input: &Schema) -> Result<Schema> {
        if self.max_rows == 0 {
            return Err(invalid("downsample max_rows must be positive"));
        }
        if !matches!(self.algorithm, Algorithm::Decimate) {
            let field = self
                .value
                .as_ref()
                .ok_or_else(|| invalid("downsampling algorithm requires value field"))?
                .field(input)?;
            if !numeric_type(field.data_type()) {
                return Err(invalid("downsampling requires numeric values"));
            }
            let minimum = if matches!(self.algorithm, Algorithm::Extrema) {
                2
            } else {
                3
            };
            if self.max_rows < minimum {
                return Err(invalid(
                    "downsampling output bound is below algorithm minimum",
                ));
            }
        }
        if matches!(self.algorithm, Algorithm::LttbLog) && self.axis.is_none() {
            return Err(invalid(
                "logarithmic LTTB requires an explicit positive numeric axis",
            ));
        }
        if let Some(axis) = &self.axis {
            let field = axis.field(input)?;
            if matches!(self.algorithm, Algorithm::LttbLog) && !numeric_type(field.data_type()) {
                return Err(invalid("logarithmic LTTB requires a numeric axis"));
            }
            if !numeric_type(field.data_type())
                && !matches!(
                    field.data_type(),
                    DataType::Timestamp(TimeUnit::Nanosecond, _)
                )
            {
                return Err(invalid("downsample axis must be numeric or a timestamp"));
            }
        }
        Ok(input.clone())
    }

    fn apply(
        &self,
        input: &RecordBatch,
        check: &mut impl FnMut() -> Result<()>,
    ) -> Result<RecordBatch> {
        if input.num_rows() <= self.max_rows {
            return Ok(input.clone());
        }
        let indices = match self.algorithm {
            Algorithm::Decimate => harmos_signal::decimate(input.num_rows(), self.max_rows),
            Algorithm::Extrema => {
                let field = self
                    .value
                    .as_ref()
                    .ok_or_else(|| invalid("missing extrema value field"))?;
                let mut values = Vec::with_capacity(input.num_rows());
                for row in 0..input.num_rows() {
                    check()?;
                    let value = number(&field.value(input, row)?)?;
                    if !value.is_finite() {
                        return Err(invalid("extrema requires finite non-null values"));
                    }
                    values.push(value);
                }
                harmos_signal::extrema(&values, self.max_rows / 2)
            }
            Algorithm::Lttb | Algorithm::LttbLog => {
                let field = self
                    .value
                    .as_ref()
                    .ok_or_else(|| invalid("missing LTTB value field"))?;
                let mut x = Vec::with_capacity(input.num_rows());
                let mut y = Vec::with_capacity(input.num_rows());
                let mut first_time = None;
                for row in 0..input.num_rows() {
                    check()?;
                    let value = number(&field.value(input, row)?)?;
                    let axis = match &self.axis {
                        None => row as f64,
                        Some(axis) => match axis.value(input, row)? {
                            ScalarValue::Datetime(value) => {
                                let first = *first_time.get_or_insert(value);
                                value
                                    .checked_sub(first)
                                    .ok_or_else(|| invalid("LTTB timestamp difference overflow"))?
                                    as f64
                            }
                            value => number(&value)?,
                        },
                    };
                    if !axis.is_finite() || !value.is_finite() {
                        return Err(invalid("LTTB requires finite non-null coordinates"));
                    }
                    if matches!(self.algorithm, Algorithm::LttbLog)
                        && (axis <= 0.0 || x.last().is_some_and(|previous| *previous > axis))
                    {
                        return Err(invalid(
                            "logarithmic LTTB requires positive ordered coordinates",
                        ));
                    }
                    x.push(axis);
                    y.push(value);
                }
                let selected = match self.algorithm {
                    Algorithm::Lttb => harmos_signal::lttb(&x, &y, self.max_rows),
                    Algorithm::LttbLog => harmos_signal::lttb_log(&x, &y, self.max_rows),
                    _ => unreachable!(),
                };
                check()?;
                selected
            }
        };
        take(
            input,
            indices.into_iter().map(|index| index as u64).collect(),
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */
pub(in crate::storage::query) fn number(value: &ScalarValue) -> Result<f64> {
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
