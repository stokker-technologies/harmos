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
use super::{Series, Summary};
use crate::{Aggregate, aggregate};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

// One member of the contract vocabulary, answered from a `Series` (every
// member) or a `Summary` (the mergeable members; median and percentiles are
// `None`, since no constant-size state can answer them).
pub trait Aggregates {
    fn aggregate(&self, kind: Aggregate) -> Option<f64>;
}

/* === Implementations === */

impl Aggregates for Series<'_> {
    fn aggregate(&self, kind: Aggregate) -> Option<f64> {
        aggregate::of_series(self, kind)
    }
}

impl Aggregates for Summary {
    fn aggregate(&self, kind: Aggregate) -> Option<f64> {
        aggregate::of_summary(self, kind)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::{Aggregate, Aggregates, Series, Summary};

    const EVERY: [Aggregate; 11] = [
        Aggregate::Count,
        Aggregate::Sum,
        Aggregate::Mean,
        Aggregate::Median,
        Aggregate::Min,
        Aggregate::Max,
        Aggregate::Rms,
        Aggregate::Std,
        Aggregate::Percentile(0),
        Aggregate::Percentile(25),
        Aggregate::Percentile(100),
    ];

    #[test]
    fn series_answers_all() {
        let series = Series::new(&[4.0, f64::NAN, 1.0, 3.0, 2.0]);
        let mut answers = Vec::new();
        for kind in EVERY {
            answers.push(series.aggregate(kind));
        }

        assert_eq!(
            answers,
            vec![
                Some(4.0),
                Some(10.0),
                Some(2.5),
                Some(2.5),
                Some(1.0),
                Some(4.0),
                series.rms(),
                series.std(),
                Some(1.0),
                Some(1.75),
                Some(4.0),
            ]
        );
        assert!((series.std().unwrap() - 1.25_f64.sqrt()).abs() < 1e-12);
        assert!((series.rms().unwrap() - 7.5_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn empty_input() {
        let series = Series::new(&[]);
        let summary = Summary::new();

        for kind in EVERY {
            let expected = matches!(kind, Aggregate::Count | Aggregate::Sum).then_some(0.0);
            assert_eq!(series.aggregate(kind), expected, "{kind:?} of a series");
            assert_eq!(summary.aggregate(kind), expected, "{kind:?} of a summary");
        }
    }

    #[test]
    fn summary_refuses_order() {
        let values = [7.5, -2.0, 3.25, 0.0, 11.0, -6.5];
        let series = Series::new(&values);
        let summary = Summary::of(&values[..2]).merge(&Summary::of(&values[2..]));

        for kind in EVERY {
            if !kind.mergeable() {
                assert_eq!(summary.aggregate(kind), None, "{kind:?} needs the values");
                assert!(series.aggregate(kind).is_some());
                continue;
            }
            let (exact, merged) = (series.aggregate(kind).unwrap(), summary.aggregate(kind).unwrap());
            assert!((exact - merged).abs() < 1e-12, "{kind:?}: {exact} and {merged}");
        }
    }
}
