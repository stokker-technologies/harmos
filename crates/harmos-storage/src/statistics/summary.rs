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

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The streaming, mergeable reduction: count, sum, extremes, mean and spread
// kept in constant space, so partial summaries of disjoint runs merge into the
// summary of their union. Non-finite values are skipped, the same values
// `Series` skips, so both answer the shared statistics identically.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    count: u64,
    sum: f64,
    mean: f64,
    lowest: f64,
    highest: f64,
    deviations: f64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Default for Summary {
    fn default() -> Self {
        Self {
            count: 0,
            sum: 0.0,
            mean: 0.0,
            lowest: f64::INFINITY,
            highest: f64::NEG_INFINITY,
            deviations: 0.0,
        }
    }
}

impl Summary {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        let before = self.mean;

        self.count += 1;
        self.sum += value;
        self.lowest = self.lowest.min(value);
        self.highest = self.highest.max(value);

        // The running mean never forms the sum, so it stays finite wherever
        // the values do; opposite signs are weighted apart so the step itself
        // cannot overflow.
        let n = self.count as f64;
        self.mean = if self.count == 1 {
            value
        } else if before.signum() == value.signum() {
            before + (value - before) / n
        } else {
            before * ((n - 1.0) / n) + value / n
        };
        self.deviations += (value - before) * (value - self.mean);
    }

    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    #[must_use]
    pub fn sum(&self) -> f64 {
        self.sum
    }

    #[must_use]
    pub fn min(&self) -> Option<f64> {
        (self.count > 0).then_some(self.lowest)
    }

    #[must_use]
    pub fn max(&self) -> Option<f64> {
        (self.count > 0).then_some(self.highest)
    }

    #[must_use]
    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then_some(self.mean)
    }

    // Population variance: the spread of exactly these values, not an
    // estimate of a wider population's.
    #[must_use]
    pub fn variance(&self) -> Option<f64> {
        (self.count > 0).then(|| (self.deviations / self.count as f64).max(0.0))
    }

    #[must_use]
    pub fn std(&self) -> Option<f64> {
        self.variance().map(f64::sqrt)
    }

    // The mean square is the squared mean plus the variance; `hypot` takes the
    // root of that sum without squaring either term into overflow.
    #[must_use]
    pub fn rms(&self) -> Option<f64> {
        Some(self.mean()?.hypot(self.std()?))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::Summary;

    #[test]
    fn running_statistics_filter_nonfinite_samples() {
        let mut summary = Summary::new();
        for value in [1.0, f64::NAN, 2.0, f64::INFINITY, 3.0, f64::NEG_INFINITY, 4.0] {
            summary.push(value);
        }
        assert_eq!(summary.count(), 4);
        assert_eq!(summary.sum(), 10.0);
        assert_eq!(summary.min(), Some(1.0));
        assert_eq!(summary.max(), Some(4.0));
        assert_eq!(summary.mean(), Some(2.5));
        assert_eq!(summary.variance(), Some(1.25));
        assert_eq!(summary.std(), Some(1.25_f64.sqrt()));
        assert!((summary.rms().unwrap() - 7.5_f64.sqrt()).abs() < 1e-12);
    }
}
