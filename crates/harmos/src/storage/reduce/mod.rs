/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(crate) mod scalar;

/* === Re-Exports === */
pub use crate::storage::reduce::scalar::{Agg, ExactNumber, ScalarKind, ScalarValue};

/* === Imports === */
// Standard Library Imports
use std::{cmp::Ordering, num::NonZeroU128};

// Third Party Imports

// Local Imports

use crate::{Error, Result};

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
    floating_mean: f64,
    scratch: Vec<ScalarValue>,
}
#[derive(Clone, Debug)]
pub struct Scratch {
    remaining: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Scratch {
    pub fn new(slots: usize) -> Self {
        Self { remaining: slots }
    }

    pub fn remaining(&self) -> usize {
        self.remaining
    }
}
impl Accumulator {
    pub fn new(agg: Agg) -> Self {
        Self {
            agg,
            rows: 0,
            count: 0,
            kind: None,
            selected: None,
            total: ExactNumber::new(false, 0, NonZeroU128::MIN),
            floating_mean: 0.0,
            scratch: Vec::new(),
        }
    }

    pub fn agg(&self) -> Agg {
        self.agg
    }

    pub fn text_bytes(&self) -> usize {
        match &self.selected {
            Some((_, ScalarValue::Text(text))) => text.capacity(),
            _ => 0,
        }
    }

    pub fn push(&mut self, capture: u64, value: &ScalarValue, scratch: &mut Scratch) -> Result<()> {
        value.check()?;
        self.rows = self
            .rows
            .checked_add(1)
            .ok_or(invalid("row count overflow"))?;
        if self.agg == Agg::Last && !matches!(value, ScalarValue::Missing) {
            let later = self
                .selected
                .as_ref()
                .is_none_or(|(previous, _)| capture > *previous);
            if later {
                self.selected = Some((capture, value.clone()));
            }
        }
        if matches!(value, ScalarValue::Missing | ScalarValue::Null) {
            return Ok(());
        }
        self.count = self
            .count
            .checked_add(1)
            .ok_or(invalid("value count overflow"))?;
        if matches!(self.agg, Agg::Len | Agg::Count | Agg::Last) {
            return Ok(());
        }
        let value_kind = value.kind().expect("checked non-missing scalar has a kind");
        match self.kind {
            Some(previous)
                if previous != value_kind && !(exact_kind(previous) && exact_kind(value_kind)) =>
            {
                return Err(invalid("mixed scalar aggregate types"));
            }
            Some(_) => {}
            None => self.kind = Some(value_kind),
        }
        match self.agg {
            Agg::Min | Agg::Max => {
                let replace = match &self.selected {
                    None => true,
                    Some((_, previous)) => {
                        let order = value.compare(previous)?;
                        if self.agg == Agg::Min {
                            order.is_lt()
                        } else {
                            order.is_gt()
                        }
                    }
                };
                if replace {
                    self.selected = Some((capture, value.clone()));
                }
            }
            Agg::Mean => {
                if let ScalarValue::Float(value) = value {
                    let n = self.count as f64;
                    self.floating_mean = if self.count == 1 {
                        *value
                    } else if self.floating_mean.signum() == value.signum() {
                        self.floating_mean + (*value - self.floating_mean) / n
                    } else {
                        self.floating_mean * ((n - 1.0) / n) + *value / n
                    };
                    if !self.floating_mean.is_finite() {
                        return Err(invalid("mean exceeds finite range"));
                    }
                } else {
                    let exact = scalar::exact(value).ok_or(invalid("mean requires numbers"))?;
                    self.total = self.total.checked_add(exact)?;
                }
            }
            Agg::Median => {
                if !matches!(
                    value_kind,
                    ScalarKind::Signed
                        | ScalarKind::Unsigned
                        | ScalarKind::Float
                        | ScalarKind::Exact
                ) {
                    return Err(invalid("median requires numbers"));
                }
                if self.scratch.len() == self.scratch.capacity() {
                    let slots = self.scratch.capacity().max(4).min(scratch.remaining);
                    if slots == 0 {
                        return Err(invalid("median values exceed admitted scratch"));
                    }
                    self.scratch
                        .try_reserve_exact(slots)
                        .map_err(|_| invalid("median scratch allocation failed"))?;
                    scratch.remaining -= slots;
                }
                self.scratch.push(value.clone());
            }
            Agg::Len | Agg::Count | Agg::Last => unreachable!("handled before type checks"),
        }
        Ok(())
    }

    pub fn finish(mut self, check: &mut impl FnMut() -> Result<()>) -> Result<ScalarValue> {
        Ok(match self.agg {
            Agg::Len => ScalarValue::Unsigned(self.rows),
            Agg::Count => ScalarValue::Unsigned(self.count),
            Agg::Min | Agg::Max | Agg::Last => self
                .selected
                .map_or(ScalarValue::Missing, |(_, value)| value),
            Agg::Mean if self.count == 0 => ScalarValue::Missing,
            Agg::Mean if self.kind == Some(ScalarKind::Float) => {
                ScalarValue::Float(self.floating_mean)
            }
            Agg::Mean => integral(self.total.divide(u128::from(self.count))?, self.kind),
            Agg::Median if self.count == 0 => ScalarValue::Missing,
            Agg::Median => {
                let mut order: Vec<usize> = Vec::new();
                order
                    .try_reserve_exact(self.scratch.len())
                    .map_err(|_| invalid("median order allocation failed"))?;
                order.extend(0..self.scratch.len());
                let scratch = &self.scratch;
                sort_values(&mut order, |a, b| {
                    check()?;
                    scratch[a].compare(&scratch[b])
                })?;
                let lower = &scratch[order[(order.len() - 1) / 2]];
                let upper = &scratch[order[order.len() / 2]];
                let value = if let (ScalarValue::Float(a), ScalarValue::Float(b)) = (lower, upper) {
                    ScalarValue::Float(if a == b {
                        *a
                    } else if a.signum() == b.signum() {
                        *a + (*b - *a) / 2.0
                    } else {
                        *a / 2.0 + *b / 2.0
                    })
                } else {
                    let midpoint = scalar::exact(lower)
                        .expect("median values are numeric")
                        .checked_add(scalar::exact(upper).expect("median values are numeric"))?
                        .divide(2)?;
                    integral(midpoint, self.kind)
                };
                self.scratch.clear();
                value
            }
        })
    }
}

/* === Public Functions === */
// Fallible In-Place Heapsort Polls Original Cancellation During Comparisons.
pub fn sort_values<T: Copy>(
    order: &mut [T],
    mut compare: impl FnMut(T, T) -> Result<Ordering>,
) -> Result<()> {
    fn sift<T: Copy>(
        order: &mut [T],
        mut root: usize,
        end: usize,
        compare: &mut impl FnMut(T, T) -> Result<Ordering>,
    ) -> Result<()> {
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
fn exact_kind(kind: ScalarKind) -> bool {
    matches!(
        kind,
        ScalarKind::Signed | ScalarKind::Unsigned | ScalarKind::Exact
    )
}
fn integral(value: ExactNumber, kind: Option<ScalarKind>) -> ScalarValue {
    if value.denominator.get() == 1 {
        if kind == Some(ScalarKind::Signed) {
            let signed = i128::try_from(value.numerator)
                .ok()
                .and_then(|n| i64::try_from(if value.negative { -n } else { n }).ok());
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
    Error::InvalidInput {
        field: "storage reduce",
        reason,
    }
}

/* === Tests === */
