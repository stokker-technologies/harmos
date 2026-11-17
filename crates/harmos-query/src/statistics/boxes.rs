/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exact Quartiles, Tukey Whiskers and Bounded Outliers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Quantiles;
use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct BoxSummary {
    pub quartiles: [f64; 3],
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub outliers: Vec<f64>,
    pub outlier_count: u64,
    bound: usize,
    fences: [f64; 2],
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl BoxSummary {
    pub fn new(quantiles: Quantiles, bound: usize, scratch_bytes: usize, check: &mut impl FnMut() -> Result<()>) -> Result<Option<Self>> {
        let ranks = quantiles.values(&[25, 50, 75], scratch_bytes, check)?;
        if ranks.is_empty() {
            return Ok(None);
        }
        let quartiles = [ranks[&25], ranks[&50], ranks[&75]];
        let scale = quartiles[0].abs().max(quartiles[2].abs());
        let fences = if scale == 0.0 {
            [0.0, 0.0]
        } else {
            let lower = quartiles[0] / scale;
            let upper = quartiles[2] / scale;
            let spread = upper - lower;
            [(lower - 1.5 * spread) * scale, (upper + 1.5 * spread) * scale]
        };
        Ok(Some(Self {
            quartiles,
            lower: None,
            upper: None,
            outliers: Vec::new(),
            outlier_count: 0,
            bound,
            fences,
        }))
    }
    pub fn push(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        if value < self.fences[0] || value > self.fences[1] {
            self.outlier_count += 1;
            if self.outliers.len() < self.bound {
                self.outliers.push(value);
            }
        } else {
            self.lower = Some(self.lower.map_or(value, |lower| lower.min(value)));
            self.upper = Some(self.upper.map_or(value, |upper| upper.max(value)));
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
