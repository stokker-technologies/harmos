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

#[cfg(test)]
mod tests {
    use super::Series;
    use crate::signal::Summary;

    const CLOSE: f64 = 1e-9;

    fn close(found: Option<f64>, expected: f64) {
        let found = found.expect("an answer");
        assert!((found - expected).abs() <= CLOSE * expected.abs().max(1.0), "{found} is not {expected}");
    }

    #[test]
    fn empty_series_undefined() {
        let series = Series::new(&[]);

        assert_eq!(series.count(), 0);
        assert_eq!(series.sum(), 0.0);
        assert_eq!(series.mean(), None);
        assert_eq!(series.median(), None);
        assert_eq!(series.min(), None);
        assert_eq!(series.max(), None);
        assert_eq!(series.rms(), None);
        assert_eq!(series.std(), None);
        assert_eq!(series.percentile(0.0), None);
        assert_eq!(series.percentile(100.0), None);
        assert!(series.all(|_| false), "no value refutes a predicate");
        assert!(!series.any(|_| true));
    }

    #[test]
    fn single_value_degenerates() {
        let series = Series::new(&[-4.0]);

        assert_eq!(series.count(), 1);
        assert_eq!(series.sum(), -4.0);
        assert_eq!(series.mean(), Some(-4.0));
        assert_eq!(series.median(), Some(-4.0));
        assert_eq!(series.min(), Some(-4.0));
        assert_eq!(series.max(), Some(-4.0));
        assert_eq!(series.rms(), Some(4.0));
        assert_eq!(series.std(), Some(0.0));
        assert_eq!(series.percentile(0.0), Some(-4.0));
        assert_eq!(series.percentile(37.0), Some(-4.0));
        assert_eq!(series.percentile(100.0), Some(-4.0));
    }

    #[test]
    fn known_run_statistics() {
        let series = Series::new(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);

        assert_eq!(series.count(), 8);
        assert_eq!(series.sum(), 40.0);
        close(series.mean(), 5.0);
        assert_eq!(series.min(), Some(2.0));
        assert_eq!(series.max(), Some(9.0));
        close(series.std(), 2.0);
        close(series.rms(), (232.0_f64 / 8.0).sqrt());
    }

    #[test]
    fn odd_even_medians() {
        assert_eq!(Series::new(&[9.0, 1.0, 5.0]).median(), Some(5.0));
        assert_eq!(Series::new(&[9.0, 1.0, 5.0, 3.0]).median(), Some(4.0));
    }

    #[test]
    fn percentiles_interpolate_ranks() {
        let series = Series::new(&[40.0, 10.0, 30.0, 20.0]);

        assert_eq!(series.percentile(0.0), Some(10.0));
        assert_eq!(series.percentile(100.0), Some(40.0));
        close(series.percentile(50.0), 25.0);
        close(series.percentile(25.0), 17.5);
        close(series.percentile(90.0), 37.0);
        assert_eq!(series.percentile(100.5), None);
        assert_eq!(series.percentile(-1.0), None);
        assert_eq!(series.percentile(f64::NAN), None);
    }

    #[test]
    fn sorting_preserves_percentiles() {
        let values = [3.0, -1.0, 8.0, 2.5, 0.0];
        let unsorted = Series::new(&values);
        let mut sorted = Series::new(&values);
        sorted.sort();

        for rank in [0.0, 12.5, 50.0, 77.0, 100.0] {
            assert_eq!(unsorted.percentile(rank), sorted.percentile(rank));
        }
    }

    #[test]
    fn extremes_avoid_overflow() {
        let series = Series::new(&[-f64::MAX, f64::MAX]);

        assert_eq!(series.median(), Some(0.0));
        assert_eq!(Series::new(&[f64::MAX, f64::MAX]).mean(), Some(f64::MAX));
    }

    #[test]
    fn non_finite_skipped() {
        let values = [1.0, f64::NAN, 3.0, f64::INFINITY, f64::NEG_INFINITY];
        let series = Series::new(&values);
        let collected: Series<'static> = values.iter().copied().collect();
        let owned = Series::from(values.to_vec());

        for series in [&series, &collected, &owned] {
            assert_eq!(series.values(), &[1.0, 3.0]);
            assert_eq!(series.count(), 2);
            assert_eq!(series.median(), Some(2.0));
            assert_eq!(series.max(), Some(3.0));
        }
        assert_eq!(Summary::of(&values), Summary::of(&[1.0, 3.0]));
    }

    #[test]
    fn predicates_bound_inclusively() {
        let series = Series::new(&[1.0, 2.5, 4.0]);

        assert!(series.all_below(4.5));
        assert!(!series.all_below(4.0), "the limit itself is not below");
        assert!(series.all_within(1.0, 4.0), "the bounds are inclusive");
        assert!(!series.all_within(1.5, 4.0));
        assert!(series.any(|value| value > 3.0));
        assert!(series.all(|value| value > 0.0));
    }
}
