/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::{cmp::Ordering, num::NonZeroU128};

use super::order_stats::Values;
use super::scalar::{self, Agg, ExactNumber, ScalarKind, ScalarValue};
use crate::aggregate;
use crate::config::StorageAccumulatorSettings;
use crate::signal::Summary;
use crate::{Aggregate, Error, Result};

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

impl Accumulator {
    pub fn new(agg: Agg) -> Self {
        Self::with_settings(agg, StorageAccumulatorSettings::default())
    }

    pub fn with_settings(agg: Agg, settings: StorageAccumulatorSettings) -> Self {
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
            Aggregate::Min | Aggregate::Max => {
                let replace = match &self.selected {
                    None => true,
                    Some((_, previous)) => {
                        let order = value.compare(previous)?;
                        if statistic == Aggregate::Min { order.is_lt() } else { order.is_gt() }
                    }
                };
                if replace {
                    self.selected = Some((capture, value.clone()));
                }
            }
            Aggregate::Sum | Aggregate::Mean => {
                if let ScalarValue::Float(value) = value {
                    self.running.push(*value);
                    let answered = aggregate::of_summary(&self.running, statistic);
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

    // Both In-Memory Sorting And Disk Merge Passes Poll Cancellation.
    fn ordered(&mut self, statistic: Aggregate, check: &mut impl FnMut() -> Result<()>) -> Result<ScalarValue> {
        let rank = match statistic {
            Aggregate::Percentile(rank) => f64::from(rank),
            _ => 50.0,
        };
        let position = rank / 100.0 * (self.scratch.length - 1) as f64;
        let (lower, upper) = self.scratch.ranks(
            position.floor() as usize,
            position.ceil() as usize,
            |a, b| Ok(approximate(a).unwrap().total_cmp(&approximate(b).unwrap())),
            check,
        )?;
        let (low, high) = (approximate(&lower).unwrap(), approximate(&upper).unwrap());
        let fraction = position - position.floor();
        let value = if fraction == 0.0 || low == high {
            low
        } else if low.signum() == high.signum() {
            low + (high - low) * fraction
        } else {
            low * (1.0 - fraction) + high * fraction
        };
        Ok(ScalarValue::Float(finite(Some(value))?))
    }

    pub fn finish(mut self, check: &mut impl FnMut() -> Result<()>) -> Result<ScalarValue> {
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
            Aggregate::Min | Aggregate::Max => self.selected.map_or(ScalarValue::Missing, |(_, value)| value),
            // The empty sum is zero, as `Summary` answers it.
            Aggregate::Sum if self.count == 0 => ScalarValue::Unsigned(0),
            Aggregate::Sum | Aggregate::Mean if floating => ScalarValue::Float(finite(aggregate::of_summary(&self.running, statistic))?),
            Aggregate::Sum => integral(self.total, self.kind),
            Aggregate::Mean if self.count == 0 => ScalarValue::Missing,
            Aggregate::Mean => integral(self.total.divide(u128::from(self.count))?, self.kind),
            Aggregate::Rms | Aggregate::Std if self.count == 0 => ScalarValue::Missing,
            Aggregate::Rms | Aggregate::Std => ScalarValue::Float(finite(aggregate::of_summary(&self.running, statistic))?),
            Aggregate::Median | Aggregate::Percentile(_) if self.count == 0 => ScalarValue::Missing,
            Aggregate::Percentile(_) => self.ordered(statistic, check)?,
            Aggregate::Median if floating => self.ordered(statistic, check)?,
            Aggregate::Median => {
                let (lower, upper) =
                    self.scratch.ranks((self.scratch.length - 1) / 2, self.scratch.length / 2, |a, b| Ok(a.compare(b)?), check)?;
                let midpoint = scalar::exact(&lower)
                    .expect("median values are numeric")
                    .checked_add(scalar::exact(&upper).expect("median values are numeric"))?
                    .divide(2)?;
                integral(midpoint, self.kind)
            }
        })
    }
}

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

fn numeric_kind(kind: ScalarKind) -> bool {
    matches!(kind, ScalarKind::Signed | ScalarKind::Unsigned | ScalarKind::Float | ScalarKind::Exact)
}
// The nearest float to a numeric scalar, for statistics that are not exact.
fn approximate(value: &ScalarValue) -> Option<f64> {
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
pub(super) fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "storage reduce", reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fold(agg: Agg, values: &[ScalarValue]) -> ScalarValue {
        let mut accumulator = Accumulator::new(agg);
        for (capture, value) in values.iter().enumerate() {
            accumulator.push(capture as u64, value).unwrap();
        }
        accumulator.finish(&mut || Ok(())).unwrap()
    }

    fn ratio(negative: bool, numerator: u128, denominator: u128) -> ScalarValue {
        ScalarValue::Exact(ExactNumber::new(negative, numerator, NonZeroU128::new(denominator).unwrap()))
    }

    #[test]
    fn public_scalar_fold() {
        let unsigned = [1u64, 2, 3, 4].map(ScalarValue::Unsigned);
        assert_eq!(fold(Agg::Of(Aggregate::Sum), &unsigned), ScalarValue::Unsigned(10));
        assert_eq!(fold(Agg::Of(Aggregate::Mean), &unsigned), ratio(false, 5, 2));
        assert_eq!(fold(Agg::Of(Aggregate::Median), &unsigned), ratio(false, 5, 2));
        let signed = [ScalarValue::Signed(-3), ScalarValue::Signed(1)];
        assert_eq!(fold(Agg::Of(Aggregate::Mean), &signed), ScalarValue::Signed(-1));
        let text = ["b", "c", "a"].map(|value| ScalarValue::Text(value.into()));
        assert_eq!(fold(Agg::Of(Aggregate::Min), &text), ScalarValue::Text("a".into()));
        assert_eq!(fold(Agg::Of(Aggregate::Max), &text), ScalarValue::Text("c".into()));
        assert_eq!(fold(Agg::Last, &text), ScalarValue::Text("a".into()));
        let sparse = [ScalarValue::Unsigned(1), ScalarValue::Null, ScalarValue::Missing];
        assert_eq!(fold(Agg::Len, &sparse), ScalarValue::Unsigned(3));
        assert_eq!(fold(Agg::Of(Aggregate::Count), &sparse), ScalarValue::Unsigned(1));
        assert_eq!(fold(Agg::Of(Aggregate::Mean), &[ScalarValue::Null]), ScalarValue::Missing);

        // Floating Statistics Answer Through The Signal Kernels: Population Std.
        let values = [1.0, 2.0, 4.0, 8.0].map(ScalarValue::Float);
        let float = |agg| match fold(Agg::Of(agg), &values) {
            ScalarValue::Float(value) => value,
            other => panic!("floating statistic answered {other:?}"),
        };
        assert!((float(Aggregate::Std) - 7.1875f64.sqrt()).abs() < 1e-12);
        assert!((float(Aggregate::Rms) - 21.25f64.sqrt()).abs() < 1e-12);
        assert_eq!(fold(Agg::Of(Aggregate::Median), &values), ScalarValue::Float(3.0));
        let mut accumulator = Accumulator::new(Agg::Of(Aggregate::Sum));
        accumulator.push(0, &ScalarValue::Unsigned(1)).unwrap();
        assert!(accumulator.push(1, &ScalarValue::Text("x".into())).is_err());
        let mut median = Accumulator::new(Agg::Of(Aggregate::Median));
        median.push(0, &ScalarValue::Unsigned(1)).unwrap();
    }
    #[test]
    fn spilled_statistics_preserve_exact_values_percentiles_and_clone_branches() {
        let settings = StorageAccumulatorSettings { memory_bytes: 0 };
        let mut median = Accumulator::with_settings(Agg::Of(Aggregate::Median), settings);
        median.push(0, &ScalarValue::Unsigned(9_007_199_254_740_993)).unwrap();
        median.push(1, &ScalarValue::Unsigned(9_007_199_254_740_995)).unwrap();
        let mut branch = median.clone();
        median.push(2, &ScalarValue::Unsigned(9_007_199_254_740_997)).unwrap();
        branch.push(2, &ScalarValue::Unsigned(9_007_199_254_740_991)).unwrap();
        assert_eq!(median.finish(&mut || Ok(())).unwrap(), ScalarValue::Unsigned(9_007_199_254_740_995));
        assert_eq!(branch.finish(&mut || Ok(())).unwrap(), ScalarValue::Unsigned(9_007_199_254_740_993));
        let values = (0..7 * 24 * 60).map(|i| ((i * 19 % 101) as f64 - 50.0).sin()).collect::<Vec<_>>();
        let series = crate::signal::Series::new(&values);
        for rank in [0, 13, 50, 77, 100] {
            let mut accumulator = Accumulator::with_settings(Agg::Of(Aggregate::Percentile(rank)), settings);
            for (index, value) in values.iter().enumerate() {
                accumulator.push(index as u64, &ScalarValue::Float(*value)).unwrap();
            }
            assert_eq!(accumulator.finish(&mut || Ok(())).unwrap(), ScalarValue::Float(series.percentile(f64::from(rank)).unwrap()));
        }
    }
}
