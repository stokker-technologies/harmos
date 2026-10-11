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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    count: u64,
    sum: f64,
    sum_sq: f64,
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
            sum_sq: 0.0,
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
        let before = self.mean().unwrap_or(0.0);

        self.count += 1;
        self.sum += value;
        self.sum_sq += value * value;
        self.lowest = self.lowest.min(value);
        self.highest = self.highest.max(value);

        let after = self.sum / self.count as f64;
        self.deviations += (value - before) * (value - after);
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
        let apart = (other.sum / other.count as f64) - (self.sum / self.count as f64);
        let weight = (self.count as f64 * other.count as f64) / count as f64;

        Self {
            count,
            sum: self.sum + other.sum,
            sum_sq: self.sum_sq + other.sum_sq,
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
    pub fn sum_sq(&self) -> f64 {
        self.sum_sq
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
        (self.count > 0).then(|| self.sum / self.count as f64)
    }

    #[must_use]
    pub fn variance(&self) -> Option<f64> {
        (self.count > 0).then(|| self.deviations / self.count as f64)
    }

    #[must_use]
    pub fn deviation(&self) -> Option<f64> {
        self.variance().map(f64::sqrt)
    }

    #[must_use]
    pub fn rms(&self) -> Option<f64> {
        (self.count > 0).then(|| (self.sum_sq / self.count as f64).sqrt())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
