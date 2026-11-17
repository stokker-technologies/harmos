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
use harmos_domain::Aggregate;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Scaled Population Moments Keep RMS And Standard Deviation Finite At Float Extremes.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Summary {
    count: u64,
    sum: f64,
    squares: f64,
    mean: f64,
    lowest: f64,
    highest: f64,
    scale: f64,
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
            squares: 0.0,
            mean: 0.0,
            lowest: f64::INFINITY,
            highest: f64::NEG_INFINITY,
            scale: 1.0,
            deviations: 0.0,
        }
    }
}

impl Summary {
    pub fn value(&self, kind: Aggregate) -> Option<f64> {
        match kind {
            Aggregate::Count => Some(self.count() as f64),
            Aggregate::Sum => Some(self.sum()),
            Aggregate::Mean => self.mean(),
            Aggregate::Min => self.min(),
            Aggregate::Max => self.max(),
            Aggregate::Rms => self.rms(),
            Aggregate::Std => self.std(),
            // A summary keeps constant state so it can merge, and an order
            // statistic is not a function of any constant state: the median of a
            // union depends on where every value falls, not on per-part counts
            // and moments. Refusing beats an estimate that reads exact.
            Aggregate::Median | Aggregate::Percentile(_) | Aggregate::First | Aggregate::Last => None,
        }
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        let maximum = value.abs().max(if self.count == 0 { 0.0 } else { self.lowest.abs().max(self.highest.abs()) });
        let scale = self.select_scale(maximum, self.count + 1);
        self.rescale(scale);
        let value_scaled = value / scale;
        let before = self.mean;
        self.count += 1;
        self.sum += value;
        self.squares += value * value;
        self.lowest = self.lowest.min(value);
        self.highest = self.highest.max(value);
        self.mean += (value_scaled - before) / self.count as f64;
        self.deviations += (value_scaled - before) * (value_scaled - self.mean);
    }

    pub fn merge(&mut self, other: &Self) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            *self = *other;
            return;
        }
        let count = self.count + other.count;
        let maximum = self.lowest.abs().max(self.highest.abs()).max(other.lowest.abs()).max(other.highest.abs());
        let scale = self.select_scale(maximum, count);
        self.rescale(scale);
        let mut other = *other;
        other.rescale(scale);
        let delta = other.mean - self.mean;
        let weight = other.count as f64 / count as f64;
        self.deviations += other.deviations + delta * delta * self.count as f64 * weight;
        self.mean += delta * weight;
        self.sum += other.sum;
        self.squares += other.squares;
        self.count = count;
        self.lowest = self.lowest.min(other.lowest);
        self.highest = self.highest.max(other.highest);
    }

    fn select_scale(&self, maximum: f64, count: u64) -> f64 {
        let safe = (f64::MAX / count as f64).sqrt() / 2.0;
        let tiny = maximum > 0.0 && maximum < f64::MIN_POSITIVE.sqrt();
        if maximum > safe || tiny { maximum } else { self.scale.max(1.0) }
    }

    fn rescale(&mut self, scale: f64) {
        let ratio = self.scale / scale;
        if self.mean != 0.0 {
            self.mean *= ratio;
        }
        if self.deviations != 0.0 {
            self.deviations *= ratio * ratio;
        }
        self.scale = scale;
    }

    pub fn is_valid(&self) -> bool {
        self.count > 0
            && self.sum.is_finite()
            && self.squares.is_finite()
            && self.squares >= 0.0
            && self.scale.is_finite()
            && self.scale > 0.0
            && self.mean.is_finite()
            && self.mean.abs() <= self.lowest.abs().max(self.highest.abs()) / self.scale
            && self.deviations.is_finite()
            && self.deviations >= 0.0
            && self.lowest.is_finite()
            && self.highest.is_finite()
            && self.lowest <= self.highest
    }

    pub fn count(&self) -> u64 {
        self.count
    }
    pub fn sum(&self) -> f64 {
        self.sum
    }
    pub fn min(&self) -> Option<f64> {
        (self.count > 0).then_some(self.lowest)
    }
    pub fn max(&self) -> Option<f64> {
        (self.count > 0).then_some(self.highest)
    }
    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then_some(self.mean * self.scale)
    }

    pub fn deviations(&self) -> f64 {
        self.deviations * self.scale * self.scale
    }
    pub fn squares(&self) -> f64 {
        self.squares
    }
    pub fn std(&self) -> Option<f64> {
        (self.count > 0).then(|| {
            self.scale * (self.deviations / self.count as f64).max(0.0).sqrt().min(self.lowest.abs().max(self.highest.abs()) / self.scale)
        })
    }
    pub fn rms(&self) -> Option<f64> {
        (self.count > 0).then(|| {
            self.scale
                * self
                    .mean
                    .hypot((self.deviations / self.count as f64).max(0.0).sqrt())
                    .min(self.lowest.abs().max(self.highest.abs()) / self.scale)
        })
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
        assert!((summary.std().unwrap().powi(2) - 1.25).abs() < 1e-12);
        assert!((summary.std().unwrap() - 1.25_f64.sqrt()).abs() < 1e-12);
        assert!((summary.rms().unwrap() - 7.5_f64.sqrt()).abs() < 1e-12);
    }
    #[test]
    fn summary_tables_and_empty_identity() {
        for (values, sum, mean, std, rms) in [
            (vec![0.0], 0.0, 0.0, 0.0, 0.0),
            (vec![3.0, 3.0, 3.0], 9.0, 3.0, 0.0, 3.0),
            (vec![-3.0, 1.0], -2.0, -1.0, 2.0, 5.0_f64.sqrt()),
            (vec![1.0, 2.0, 3.0, 4.0], 10.0, 2.5, 1.25_f64.sqrt(), 7.5_f64.sqrt()),
        ] {
            let mut summary = Summary::new();
            for value in &values {
                summary.push(*value);
            }
            assert_eq!(summary.count(), values.len() as u64);
            assert_eq!(summary.sum(), sum);
            for (actual, expected) in [(summary.mean(), mean), (summary.std(), std), (summary.rms(), rms)] {
                assert!((actual.unwrap() - expected).abs() < 1e-12);
            }
            assert_eq!(summary.min(), values.iter().copied().reduce(f64::min));
            assert_eq!(summary.max(), values.iter().copied().reduce(f64::max));
        }
        let mut empty = Summary::new();
        empty.push(f64::NAN);
        empty.push(f64::INFINITY);
        assert_eq!((empty.count(), empty.sum()), (0, 0.0));
        assert_eq!([empty.mean(), empty.min(), empty.max(), empty.rms(), empty.std()], [None; 5]);
        let tiny = f64::from_bits(1);
        let mut tiny_summary = Summary::new();
        tiny_summary.push(-tiny);
        tiny_summary.push(tiny);
        assert_eq!(tiny_summary.rms(), Some(tiny));
        assert_eq!(tiny_summary.std(), Some(tiny));
        let mut zero = Summary::new();
        zero.push(0.0);
        zero.merge(&tiny_summary);
        assert_eq!(zero.rms(), Some(tiny));
        for values in [[f64::MAX, f64::MAX], [-f64::MAX, f64::MAX]] {
            let mut summary = Summary::new();
            for value in values {
                summary.push(value);
            }
            assert_eq!(summary.rms(), Some(f64::MAX));
            assert_eq!(summary.std(), Some(if values[0] == values[1] { 0.0 } else { f64::MAX }));
        }
    }

    #[test]
    fn summaries_merge_associatively_over_adjacent_ranges() {
        let mut parts = [Summary::new(); 3];
        let mut whole = Summary::new();
        for (index, value) in [-9.0, 3.0, 12.0, 4.0, -7.0, 0.0, 2.0, 8.0, 5.0].into_iter().enumerate() {
            parts[index / 3].push(value);
            whole.push(value);
        }
        let mut left = parts[0];
        left.merge(&parts[1]);
        left.merge(&parts[2]);
        let mut right = parts[1];
        right.merge(&parts[2]);
        let mut grouped = parts[0];
        grouped.merge(&right);
        for summary in [left, grouped] {
            assert_eq!(summary.count(), whole.count());
            assert_eq!((summary.min(), summary.max(), summary.sum()), (whole.min(), whole.max(), whole.sum()));
            for (actual, expected) in [(summary.mean(), whole.mean()), (summary.std(), whole.std()), (summary.rms(), whole.rms())] {
                assert!((actual.unwrap() - expected.unwrap()).abs() < 1e-12);
            }
        }
    }
}
