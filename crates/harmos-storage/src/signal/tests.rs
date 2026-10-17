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

/* === Constants === */
const CLOSE: f64 = 1e-9;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn close(found: Option<f64>, expected: f64) {
    let found = found.expect("an answer");
    assert!(
        (found - expected).abs() <= CLOSE * expected.abs().max(1.0),
        "{found} is not {expected}"
    );
}

/* === Tests === */

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

// Series answers over the whole run at once; Summary is folded from chunks and
// merged, so agreement proves the streaming form matches the exact one.
#[test]
fn chunked_summaries_agree() {
    let values: Vec<f64> = (0..97)
        .map(|index| ((index * 37) % 23) as f64 * 1.5 - 11.0)
        .chain([f64::NAN, f64::INFINITY])
        .collect();
    let series = Series::new(&values);
    let summary = values
        .chunks(10)
        .map(Summary::of)
        .fold(Summary::new(), |held, next| held.merge(&next));

    assert_eq!(series.count(), summary.count());
    close(Some(series.sum()), summary.sum());
    close(series.mean(), summary.mean().unwrap());
    assert_eq!(series.min(), summary.min());
    assert_eq!(series.max(), summary.max());
    close(series.rms(), summary.rms().unwrap());
    close(series.std(), summary.std().unwrap());
}
