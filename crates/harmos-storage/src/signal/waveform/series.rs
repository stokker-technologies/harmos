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
use std::borrow::Cow;

// Third Party Imports

// Local Imports
use super::Summary;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// A finished run of values answered exactly: everything `Summary` answers,
// plus the order statistics that need every value at once. Non-finite values
// are skipped on construction, so a slice that is already finite is borrowed
// and only a slice holding NaN or an infinity is copied.
#[derive(Clone, Debug, PartialEq)]
pub struct Series<'a> {
    values: Cow<'a, [f64]>,
    sorted: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<'a> Series<'a> {
    #[must_use]
    pub fn new(values: &'a [f64]) -> Self {
        let values =
            if values.iter().all(|value| value.is_finite()) { Cow::Borrowed(values) } else { Cow::Owned(finite(values.iter().copied())) };

        Self { values, sorted: false }
    }

    // Orders the values in place, so every later median and percentile reads
    // them without copying. A borrowed slice is copied once here.
    pub fn sort(&mut self) {
        if !self.sorted {
            self.values.to_mut().sort_by(f64::total_cmp);
            self.sorted = true;
        }
    }

    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    #[must_use]
    pub fn summary(&self) -> Summary {
        Summary::of(&self.values)
    }

    #[must_use]
    pub fn count(&self) -> u64 {
        self.values.len() as u64
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use]
    pub fn sum(&self) -> f64 {
        self.summary().sum()
    }

    #[must_use]
    pub fn mean(&self) -> Option<f64> {
        self.summary().mean()
    }

    #[must_use]
    pub fn min(&self) -> Option<f64> {
        self.summary().min()
    }

    #[must_use]
    pub fn max(&self) -> Option<f64> {
        self.summary().max()
    }

    #[must_use]
    pub fn rms(&self) -> Option<f64> {
        self.summary().rms()
    }

    // Population standard deviation, the same spread `Summary::std` answers.
    #[must_use]
    pub fn std(&self) -> Option<f64> {
        self.summary().std()
    }

    #[must_use]
    pub fn median(&self) -> Option<f64> {
        self.percentile(50.0)
    }

    // Linear interpolation between the two closest ranks: rank `p / 100 *
    // (n - 1)` over the ascending values, so 0 is the minimum, 100 the maximum
    // and 50 the median of an even count is the midpoint of the middle pair.
    // A rank outside 0..=100 has no answer.
    #[must_use]
    pub fn percentile(&self, rank: f64) -> Option<f64> {
        if self.values.is_empty() || !(0.0..=100.0).contains(&rank) {
            return None;
        }
        let ordered: Cow<'_, [f64]> = if self.sorted {
            Cow::Borrowed(&self.values)
        } else {
            let mut copy = self.values.to_vec();
            copy.sort_by(f64::total_cmp);
            Cow::Owned(copy)
        };

        let position = rank / 100.0 * (ordered.len() - 1) as f64;
        let lower = position.floor() as usize;
        let upper = position.ceil() as usize;

        Some(between(ordered[lower], ordered[upper], position - lower as f64))
    }

    // Every value satisfies the predicate; vacuously true for no values, the
    // way `Iterator::all` is, so a caller that needs evidence checks `count`.
    pub fn all(&self, mut holds: impl FnMut(f64) -> bool) -> bool {
        self.values.iter().all(|value| holds(*value))
    }

    pub fn any(&self, mut holds: impl FnMut(f64) -> bool) -> bool {
        self.values.iter().any(|value| holds(*value))
    }

    #[must_use]
    pub fn all_below(&self, limit: f64) -> bool {
        self.all(|value| value < limit)
    }

    #[must_use]
    pub fn all_within(&self, min: f64, max: f64) -> bool {
        self.all(|value| (min..=max).contains(&value))
    }
}

impl<'a> From<&'a [f64]> for Series<'a> {
    fn from(values: &'a [f64]) -> Self {
        Self::new(values)
    }
}

impl From<Vec<f64>> for Series<'static> {
    fn from(mut values: Vec<f64>) -> Self {
        values.retain(|value| value.is_finite());
        Self { values: Cow::Owned(values), sorted: false }
    }
}

impl FromIterator<f64> for Series<'static> {
    fn from_iter<I: IntoIterator<Item = f64>>(values: I) -> Self {
        Self { values: Cow::Owned(finite(values)), sorted: false }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn finite(values: impl IntoIterator<Item = f64>) -> Vec<f64> {
    values.into_iter().filter(|value| value.is_finite()).collect()
}

// A point a fraction of the way from one value to the next that stays finite
// for any finite pair: the difference is formed only when both share a sign.
fn between(low: f64, high: f64, fraction: f64) -> f64 {
    if fraction == 0.0 || low == high {
        low
    } else if low.signum() == high.signum() {
        low + (high - low) * fraction
    } else {
        low * (1.0 - fraction) + high * fraction
    }
}

/* === Tests === */
