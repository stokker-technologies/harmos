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
use crate::signal::{Series, Summary};

// Local Imports
use super::scalar::{self, Agg, ExactNumber, ScalarKind, ScalarValue};
use crate::aggregate;
use crate::{Aggregate, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
/// Folds scalar values into one aggregate with query transform semantics.
///
/// Exact columns (signed, unsigned and exact fractions) keep `Sum`, `Mean` and
/// `Median` exact in rational arithmetic, and `Min`, `Max` and `Last` select by
/// scalar ordering over any comparable kind; neither is floating statistics.
/// Every floating statistic is answered by the signal `Summary` (mergeable
/// members) or `Series` (median and percentiles); `Rms`, `Std` and
/// percentiles of exact columns read their values as the nearest float.
/// `Missing` and `Null` count toward `Len` only.
#[derive(Clone, Debug)]
pub struct Accumulator {
    agg: Agg,
    rows: u64,
    count: u64,
    kind: Option<ScalarKind>,
    selected: Option<(u64, ScalarValue)>,
    total: ExactNumber,
    running: Summary,
    scratch: Vec<ScalarValue>,
}
/// The bound on values order statistics retain, shared by the accumulators of one fold.
#[derive(Clone, Debug)]
pub struct Scratch {
    remaining: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Scratch {
    /// Admit at most `slots` retained values across every accumulator using it.
    pub fn new(slots: usize) -> Self {
        Self { remaining: slots }
    }
}
impl Accumulator {
    /// An empty fold of `agg`.
    pub fn new(agg: Agg) -> Self {
        Self {
            agg,
            rows: 0,
            count: 0,
            kind: None,
            selected: None,
            total: ExactNumber::new(false, 0, NonZeroU128::MIN),
            running: Summary::new(),
            scratch: Vec::new(),
        }
    }

    /// Fold one value observed at `capture` order; `Last` keeps the greatest capture.
    pub fn push(&mut self, capture: u64, value: &ScalarValue, scratch: &mut Scratch) -> Result<()> {
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
                if self.scratch.len() == self.scratch.capacity() {
                    let slots = self.scratch.capacity().max(4).min(scratch.remaining);
                    if slots == 0 {
                        return Err(invalid("order statistic values exceed admitted scratch"));
                    }
                    self.scratch.try_reserve_exact(slots).map_err(|_| invalid("order statistic scratch allocation failed"))?;
                    scratch.remaining -= slots;
                }
                self.scratch.push(value.clone());
            }
            Aggregate::Count => unreachable!("handled before type checks"),
        }
        Ok(())
    }

    // Median and percentiles through `Series`, polling cancellation around
    // the one sort that orders every retained value.
    fn ordered(&mut self, statistic: Aggregate, check: &mut impl FnMut() -> Result<()>) -> Result<ScalarValue> {
        let mut values: Vec<f64> = Vec::new();
        values.try_reserve_exact(self.scratch.len()).map_err(|_| invalid("order statistic allocation failed"))?;
        values.extend(self.scratch.iter().map(|value| approximate(value).expect("order statistic values are numeric")));
        self.scratch = Vec::new();
        check()?;
        let mut series = Series::from(values);
        series.sort();
        check()?;
        Ok(ScalarValue::Float(finite(aggregate::of_series(&series, statistic))?))
    }

    /// The aggregate, polling `check` around the sorts order statistics need.
    /// An empty fold answers `Missing`, except `Len`, `Count` and `Sum`.
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
                let mut order: Vec<usize> = Vec::new();
                order.try_reserve_exact(self.scratch.len()).map_err(|_| invalid("median order allocation failed"))?;
                order.extend(0..self.scratch.len());
                let scratch = &self.scratch;
                sort_values(&mut order, |a, b| {
                    check()?;
                    Ok(scratch[a].compare(&scratch[b])?)
                })?;
                let lower = &scratch[order[(order.len() - 1) / 2]];
                let upper = &scratch[order[order.len() / 2]];
                let midpoint = scalar::exact(lower)
                    .expect("median values are numeric")
                    .checked_add(scalar::exact(upper).expect("median values are numeric"))?
                    .divide(2)?;
                self.scratch.clear();
                integral(midpoint, self.kind)
            }
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

/* === Tests === */
