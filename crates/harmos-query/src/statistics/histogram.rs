/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Statistics Contracts and Execution

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::Result;
use crate::transform::invalid;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Histogram {
    edges: Vec<f64>,
    counts: Vec<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Histogram {
    pub fn new(edges: Vec<f64>) -> Result<Self> {
        if edges.len() < 2
            || edges.len() > 65536
            || edges.iter().any(|edge| !edge.is_finite())
            || edges.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid("supply 2..65536 finite strictly increasing histogram edges"));
        }
        Ok(Self { counts: vec![0; edges.len() + 1], edges })
    }
    pub fn push(&mut self, value: f64) -> Result<()> {
        if !value.is_finite() {
            return Ok(());
        }
        let bin = if value == self.edges[self.edges.len() - 1] {
            self.edges.len() - 1
        } else {
            self.edges.partition_point(|edge| *edge <= value)
        };
        self.counts[bin] = self.counts[bin].checked_add(1).ok_or_else(|| invalid("reduce histogram input to avoid count overflow"))?;
        Ok(())
    }
    pub fn counts(self) -> Vec<u64> {
        self.counts
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::Histogram;
    #[test]
    fn histogram_counts_edges_overflow_bins_and_empty_input() {
        let mut histogram = Histogram::new(vec![0.0, 1.0, 2.0]).unwrap();
        for value in [-1.0, 0.0, 0.5, 1.0, 2.0, 3.0, f64::NAN, f64::INFINITY] {
            histogram.push(value).unwrap();
        }
        assert_eq!(histogram.counts(), vec![1, 2, 2, 1]);
        assert_eq!(Histogram::new(vec![0.0, 1.0, 2.0]).unwrap().counts(), vec![0; 4]);
        for edges in [vec![], vec![1.0], vec![1.0, 1.0], vec![1.0, 0.0], vec![0.0, f64::INFINITY]] {
            assert!(Histogram::new(edges).is_err());
        }
    }
}
