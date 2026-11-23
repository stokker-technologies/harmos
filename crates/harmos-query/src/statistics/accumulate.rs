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

/* === Imports === */

// Standard Library Imports
use std::{cmp::Ordering, num::NonZeroU128};

// Third Party Imports

// Local Imports
use super::order_stats::Values;
use crate::config::QueryAccumulatorSettings;
use crate::scalar::{self, Agg, ExactNumber, ScalarKind, ScalarValue};
use crate::statistics::Summary;
use crate::{Aggregate, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct Accumulator {
    agg: Agg,
    rows: u64,
    count: u64,
    kind: Option<ScalarKind>,
    selected: Option<(u64, ScalarValue)>,
    total: ExactNumber,
    running: Summary,
    scratch: Values,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Accumulator {
    pub fn new(agg: Agg) -> Self {
        Self::with_settings(agg, QueryAccumulatorSettings::default())
    }

    pub fn with_settings(agg: Agg, settings: QueryAccumulatorSettings) -> Self {
        Self {
            agg,
            rows: 0,
            count: 0,
            kind: None,
            selected: None,
            total: ExactNumber::new(false, 0, NonZeroU128::MIN),
            running: Summary::new(),
            scratch: Values::new(settings),
        }
    }

    pub fn push(&mut self, capture: u64, value: &ScalarValue) -> Result<()> {
        // A Nonfinite Sample Is a Missing Measurement.
        let value = if matches!(value, ScalarValue::Float(value) if !value.is_finite()) { &ScalarValue::Null } else { value };
        value.check()?;
        self.rows = self.rows.checked_add(1).ok_or(invalid("row count overflow"))?;
        if self.agg == Agg::Last && !matches!(value, ScalarValue::Missing) {
            let later = self.selected.as_ref().is_none_or(|(previous, _)| capture > *previous);
            if later {
                self.selected = Some((capture, value.clone()));
            }
        }
        if matches!(value, ScalarValue::Missing | ScalarValue::Null) {
            return Ok(());
        }
        self.count = self.count.checked_add(1).ok_or(invalid("value count overflow"))?;
        let Agg::Of(statistic) = self.agg else {
            return Ok(());
        };
        if statistic == Aggregate::Count {
            return Ok(());
        }
        let value_kind = value.kind().expect("checked non-missing scalar has a kind");
        match self.kind {
            Some(previous) if previous != value_kind && !(exact_kind(previous) && exact_kind(value_kind)) => {
                return Err(invalid("mixed scalar aggregate types"));
            }
            Some(_) => {}
            None => self.kind = Some(value_kind),
        }
        match statistic {
            Aggregate::First | Aggregate::Last | Aggregate::Min | Aggregate::Max => {
                let replace = match &self.selected {
                    None => true,
                    Some((previous_capture, previous)) => {
                        let order = value.compare(previous)?;
                        match statistic {
                            Aggregate::First => capture < *previous_capture,
                            Aggregate::Last => capture > *previous_capture,
                            Aggregate::Min => order.is_lt(),
                            _ => order.is_gt(),
                        }
                    }
                };
                if replace {
                    self.selected = Some((capture, value.clone()));
                }
            }
            Aggregate::Sum | Aggregate::Mean => {
                if let ScalarValue::Float(value) = value {
                    self.running.push(*value);
                    let answered = self.running.value(statistic);
                    if !answered.is_some_and(f64::is_finite) {
                        return Err(invalid("aggregate exceeds finite range"));
                    }
                } else {
                    let exact = scalar::exact(value).ok_or(invalid("aggregate requires numbers"))?;
                    self.total = self.total.checked_add(exact)?;
                }
            }
            Aggregate::Rms | Aggregate::Std => {
                self.running.push(approximate(value).ok_or(invalid("aggregate requires numbers"))?);
            }
            Aggregate::Median | Aggregate::Percentile(_) => {
                if !numeric_kind(value_kind) {
                    return Err(invalid("order statistic requires numbers"));
                }
                self.scratch.push(value)?;
            }
            Aggregate::Count => unreachable!("handled before type checks"),
        }
        Ok(())
    }

    /// Finish Several Order Statistics From One Accumulation.
    /// Integer Medians Retain Exact Arithmetic; Percentiles Use Floating Interpolation.
    pub fn finish_ranks(mut self, statistics: &[Aggregate], check: &mut impl FnMut() -> Result<()>) -> Result<Vec<ScalarValue>> {
        check()?;
        if !matches!(self.agg, Agg::Of(Aggregate::Median | Aggregate::Percentile(_)))
            || statistics.iter().any(|statistic| !matches!(statistic, Aggregate::Median | Aggregate::Percentile(0..=100)))
        {
            return Err(invalid("select median or percentile ranks in 0..=100"));
        }
        if self.count == 0 {
            return Ok(vec![ScalarValue::Missing; statistics.len()]);
        }
        let mut ranks = Vec::with_capacity(statistics.len() * 2);
        for statistic in statistics {
            let rank = if let Aggregate::Percentile(rank) = statistic { *rank } else { 50 };
            let numerator = (self.scratch.length - 1) as u128 * u128::from(rank);
            let lower = (numerator / 100) as usize;
            ranks.push(lower);
            ranks.push(lower + usize::from(!numerator.is_multiple_of(100)));
        }
        let values = self.scratch.ranks(&ranks, check)?;
        statistics
            .iter()
            .zip(values.as_chunks::<2>().0)
            .map(|(statistic, values)| {
                if *statistic == Aggregate::Median && self.kind != Some(ScalarKind::Float) {
                    let midpoint = scalar::exact(&values[0])
                        .expect("numeric median")
                        .checked_add(scalar::exact(&values[1]).expect("numeric median"))?
                        .divide(2)?;
                    return Ok(integral(midpoint, self.kind));
                }
                let rank = if let Aggregate::Percentile(rank) = statistic { *rank } else { 50 };
                let numerator = (self.scratch.length - 1) as u128 * u128::from(rank);
                let fraction = (numerator % 100) as f64 / 100.0;
                let low = approximate(&values[0]).expect("numeric rank");
                let high = approximate(&values[1]).expect("numeric rank");
                let answer = if low == high {
                    low
                } else if low.signum() == high.signum() {
                    low + (high - low) * fraction
                } else {
                    low * (1.0 - fraction) + high * fraction
                };
                Ok(ScalarValue::Float(finite(Some(answer))?))
            })
            .collect()
    }

    pub fn finish(self, check: &mut impl FnMut() -> Result<()>) -> Result<ScalarValue> {
        let statistic = match self.agg {
            Agg::Len => return Ok(ScalarValue::Unsigned(self.rows)),
            Agg::Last => {
                return Ok(self.selected.map_or(ScalarValue::Missing, |(_, value)| value));
            }
            Agg::Of(statistic) => statistic,
        };
        let floating = self.kind == Some(ScalarKind::Float);
        Ok(match statistic {
            Aggregate::Count => ScalarValue::Unsigned(self.count),
            Aggregate::First | Aggregate::Last | Aggregate::Min | Aggregate::Max => {
                self.selected.map_or(ScalarValue::Missing, |(_, value)| value)
            }
            // The empty sum is zero, as `Summary` answers it.
            Aggregate::Sum if self.count == 0 => ScalarValue::Unsigned(0),
            Aggregate::Sum | Aggregate::Mean if floating => ScalarValue::Float(finite(self.running.value(statistic))?),
            Aggregate::Sum => integral(self.total, self.kind),
            Aggregate::Mean if self.count == 0 => ScalarValue::Missing,
            Aggregate::Mean => integral(self.total.divide(u128::from(self.count))?, self.kind),
            Aggregate::Rms | Aggregate::Std if self.count == 0 => ScalarValue::Missing,
            Aggregate::Rms | Aggregate::Std => ScalarValue::Float(finite(self.running.value(statistic))?),
            Aggregate::Median | Aggregate::Percentile(_) if self.count == 0 => ScalarValue::Missing,
            Aggregate::Median | Aggregate::Percentile(_) => self.finish_ranks(&[statistic], check)?.remove(0),
        })
    }
}

/* === Public Functions === */

// Fallible In-Place Heapsort Polls Original Cancellation During Comparisons.
pub fn sort_values<T: Copy>(order: &mut [T], mut compare: impl FnMut(T, T) -> Result<Ordering>) -> Result<()> {
    fn sift<T: Copy>(order: &mut [T], mut root: usize, end: usize, compare: &mut impl FnMut(T, T) -> Result<Ordering>) -> Result<()> {
        while root < end / 2 {
            let mut child = root * 2 + 1;
            if child + 1 < end && compare(order[child], order[child + 1])?.is_lt() {
                child += 1;
            }
            if !compare(order[root], order[child])?.is_lt() {
                break;
            }
            order.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..order.len() / 2).rev() {
        sift(order, root, order.len(), &mut compare)?;
    }
    for end in (1..order.len()).rev() {
        order.swap(0, end);
        sift(order, 0, end, &mut compare)?;
    }
    Ok(())
}

/* === Private Functions === */

fn numeric_kind(kind: ScalarKind) -> bool {
    matches!(kind, ScalarKind::Signed | ScalarKind::Unsigned | ScalarKind::Float | ScalarKind::Exact)
}
// The nearest float to a numeric scalar, for statistics that are not exact.
pub(crate) fn approximate(value: &ScalarValue) -> Option<f64> {
    if let ScalarValue::Float(value) = value {
        return Some(*value);
    }
    let exact = scalar::exact(value)?;
    let magnitude = exact.numerator as f64 / exact.denominator.get() as f64;
    Some(if exact.negative { -magnitude } else { magnitude })
}
fn finite(answer: Option<f64>) -> Result<f64> {
    answer.filter(|value| value.is_finite()).ok_or(invalid("aggregate exceeds finite range"))
}
fn exact_kind(kind: ScalarKind) -> bool {
    matches!(kind, ScalarKind::Signed | ScalarKind::Unsigned | ScalarKind::Exact)
}
fn integral(value: ExactNumber, kind: Option<ScalarKind>) -> ScalarValue {
    if value.denominator.get() == 1 {
        if kind == Some(ScalarKind::Signed) {
            let signed = i128::try_from(value.numerator).ok().and_then(|n| i64::try_from(if value.negative { -n } else { n }).ok());
            if let Some(signed) = signed {
                return ScalarValue::Signed(signed);
            }
        } else if !value.negative
            && let Ok(unsigned) = u64::try_from(value.numerator)
        {
            return ScalarValue::Unsigned(unsigned);
        }
    }
    ScalarValue::Exact(value)
}
pub(crate) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage reduce", reason }
}

/* === Tests === */
