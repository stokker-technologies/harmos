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
use harmos::Aggregate;
use harmos::signal::{Aggregates, Series, Summary};

// Local Imports

/* === Constants === */
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

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn series_answers_all() {
    let series = Series::new(&[4.0, f64::NAN, 1.0, 3.0, 2.0]);
    let answers: Vec<Option<f64>> = EVERY.iter().map(|kind| series.aggregate(*kind)).collect();

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

pub fn empty_input() {
    let series = Series::new(&[]);
    let summary = Summary::new();

    for kind in EVERY {
        let expected = matches!(kind, Aggregate::Count | Aggregate::Sum).then_some(0.0);
        assert_eq!(series.aggregate(kind), expected, "{kind:?} of a series");
        assert_eq!(summary.aggregate(kind), expected, "{kind:?} of a summary");
    }
}

pub fn summary_refuses_order() {
    let values = [7.5, -2.0, 3.25, 0.0, 11.0, -6.5];
    let series = Series::new(&values);
    let summary = Summary::of(&values[..2]).merge(&Summary::of(&values[2..]));

    for kind in EVERY {
        if kind.mergeable() {
            let (exact, merged) = (
                series.aggregate(kind).unwrap(),
                summary.aggregate(kind).unwrap(),
            );
            assert!(
                (exact - merged).abs() < 1e-12,
                "{kind:?}: {exact} and {merged}"
            );
        } else {
            assert_eq!(summary.aggregate(kind), None, "{kind:?} needs the values");
            assert!(series.aggregate(kind).is_some());
        }
    }
}

// The wire forms: snake-case serde, `p95` names, and ranks past 100 refused.
pub fn wire_names() {
    for kind in EVERY {
        assert_eq!(Aggregate::parse(&kind.name()), Some(kind));
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(serde_json::from_str::<Aggregate>(&json).unwrap(), kind);
    }
    assert_eq!(serde_json::to_string(&Aggregate::Rms).unwrap(), "\"rms\"");
    assert_eq!(
        serde_json::to_string(&Aggregate::Percentile(95)).unwrap(),
        "{\"percentile\":95}"
    );
    assert_eq!(Aggregate::Percentile(95).name(), "p95");
    for refused in ["p101", "p", "p+5", "variance"] {
        assert_eq!(Aggregate::parse(refused), None, "{refused}");
    }
    assert!(!Aggregate::Percentile(101).answerable());
}

/* === Private Functions === */

/* === Tests === */
