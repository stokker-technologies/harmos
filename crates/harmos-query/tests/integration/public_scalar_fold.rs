/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Deterministic Statistics Integration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroU128;

// Third Party Imports

// Local Imports
use harmos_query::scalar::ExactNumber;
use harmos_query::scalar::{Agg, ScalarValue};
use harmos_query::{Accumulator, Aggregate};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn fold(agg: Agg, values: &[ScalarValue]) -> ScalarValue {
    let mut accumulator = Accumulator::new(agg);
    for (capture, value) in values.iter().enumerate() {
        accumulator.push(capture as u64, value).unwrap();
    }
    accumulator.finish(&mut || Ok(())).unwrap()
}

fn ratio(negative: bool, numerator: u128, denominator: u128) -> ScalarValue {
    ScalarValue::Exact(ExactNumber::new(negative, numerator, NonZeroU128::new(denominator).unwrap()))
}

/* === Tests === */

#[test]
fn public_scalar_fold() {
    let unsigned = [1u64, 2, 3, 4].map(ScalarValue::Unsigned);
    assert_eq!(fold(Agg::Of(Aggregate::Sum), &unsigned), ScalarValue::Unsigned(10));
    assert_eq!(fold(Agg::Of(Aggregate::Mean), &unsigned), ratio(false, 5, 2));
    assert_eq!(fold(Agg::Of(Aggregate::Median), &unsigned), ratio(false, 5, 2));
    let signed = [ScalarValue::Signed(-3), ScalarValue::Signed(1)];
    assert_eq!(fold(Agg::Of(Aggregate::Mean), &signed), ScalarValue::Signed(-1));
    let text = ["b", "c", "a"].map(|value| ScalarValue::Text(value.into()));
    assert_eq!(fold(Agg::Of(Aggregate::Min), &text), ScalarValue::Text("a".into()));
    assert_eq!(fold(Agg::Of(Aggregate::Max), &text), ScalarValue::Text("c".into()));
    assert_eq!(fold(Agg::Last, &text), ScalarValue::Text("a".into()));
    let sparse = [ScalarValue::Unsigned(1), ScalarValue::Null, ScalarValue::Missing];
    assert_eq!(fold(Agg::Len, &sparse), ScalarValue::Unsigned(3));
    assert_eq!(fold(Agg::Of(Aggregate::Count), &sparse), ScalarValue::Unsigned(1));
    assert_eq!(fold(Agg::Of(Aggregate::Mean), &[ScalarValue::Null]), ScalarValue::Missing);

    // Floating Statistics Answer Through The Signal Kernels: Population Std.
    let values = [1.0, 2.0, 4.0, 8.0].map(ScalarValue::Float);
    let float = |agg| match fold(Agg::Of(agg), &values) {
        ScalarValue::Float(value) => value,
        other => panic!("floating statistic answered {other:?}"),
    };
    assert!((float(Aggregate::Std) - 7.1875f64.sqrt()).abs() < 1e-12);
    assert!((float(Aggregate::Rms) - 21.25f64.sqrt()).abs() < 1e-12);
    assert_eq!(fold(Agg::Of(Aggregate::Median), &values), ScalarValue::Float(3.0));
    let mut accumulator = Accumulator::new(Agg::Of(Aggregate::Sum));
    accumulator.push(0, &ScalarValue::Unsigned(1)).unwrap();
    assert!(accumulator.push(1, &ScalarValue::Text("x".into())).is_err());
    let mut median = Accumulator::new(Agg::Of(Aggregate::Median));
    median.push(0, &ScalarValue::Unsigned(1)).unwrap();
}
