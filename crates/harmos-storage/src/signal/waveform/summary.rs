/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

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

    #[must_use]
    pub fn of(values: &[f64]) -> Self {
        let mut summary = Self::new();
        for value in values {
            summary.push(*value);
        }
        summary
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
    pub fn merge(&self, other: &Self) -> Self {
        if self.count == 0 {
            return *other;
        }
        if other.count == 0 {
            return *self;
        }

        let count = self.count + other.count;
        let total = count as f64;
        let mean = self.mean * (self.count as f64 / total) + other.mean * (other.count as f64 / total);
        let apart = other.mean - self.mean;
        let weight = (self.count as f64 * other.count as f64) / total;

        Self {
            count,
            sum: self.sum + other.sum,
            mean,
            lowest: self.lowest.min(other.lowest),
            highest: self.highest.max(other.highest),
            deviations: self.deviations + other.deviations + apart * apart * weight,
        }
    }

    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
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
    pub fn span(&self) -> Option<f64> {
        (self.count > 0).then_some(self.highest - self.lowest)
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

#[cfg(test)]
mod tests {
    use super::Summary;
    use crate::signal::Series;

    const CLOSE: f64 = 1e-9;

    fn close(found: Option<f64>, expected: f64) {
        let found = found.expect("an answer");
        assert!((found - expected).abs() <= CLOSE * expected.abs().max(1.0), "{found} is not {expected}");
    }

    // Series answers over the whole run at once; Summary is folded from chunks and
    // merged, so agreement proves the streaming form matches the exact one.
    #[test]
    fn chunked_summaries_agree() {
        let values: Vec<f64> = (0..97).map(|index| ((index * 37) % 23) as f64 * 1.5 - 11.0).chain([f64::NAN, f64::INFINITY]).collect();
        let series = Series::new(&values);
        let summary = values.chunks(10).map(Summary::of).fold(Summary::new(), |held, next| held.merge(&next));

        assert_eq!(series.count(), summary.count());
        close(Some(series.sum()), summary.sum());
        close(series.mean(), summary.mean().unwrap());
        assert_eq!(series.min(), summary.min());
        assert_eq!(series.max(), summary.max());
        close(series.rms(), summary.rms().unwrap());
        close(series.std(), summary.std().unwrap());
    }
}
