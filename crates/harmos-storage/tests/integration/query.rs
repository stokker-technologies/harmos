/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Reproduce Small Capture Order Statistics

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_storage::query::{
    Accumulator,
    scalar::{Agg, ScalarValue},
};
use harmos_storage::{Aggregate, Error, StorageAccumulatorSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn capture_statistics() {
    let settings = StorageAccumulatorSettings { memory_bytes: 4_000 };
    for n in [2, 3, 4, 11, 13, 45, 100, 101] {
        for (statistic, fraction) in [(Aggregate::Median, 0.5), (Aggregate::Percentile(95), 0.95), (Aggregate::Percentile(99), 0.99)] {
            let mut memory = Accumulator::new(Agg::Of(statistic));
            let mut bounded = Accumulator::with_settings(Agg::Of(statistic), settings);
            for (capture, value) in (0..n).rev().enumerate() {
                let value = ScalarValue::Float(value as f64);
                memory.push(capture as u64, &value).unwrap();
                bounded.push(capture as u64, &value).unwrap();
            }
            let answer = memory.finish(&mut || Ok(())).unwrap();
            assert_eq!(bounded.finish(&mut || Ok(())).unwrap(), answer);
            assert_eq!(answer, ScalarValue::Float((n - 1) as f64 * fraction));
            if n == 100 {
                let expected = match statistic {
                    Aggregate::Median => 49.5,
                    Aggregate::Percentile(95) => 94.05,
                    _ => 98.01,
                };
                assert_eq!(answer, ScalarValue::Float(expected));
            }
        }
    }
}

#[test]
fn cancellation_and_finiteness() {
    for memory_bytes in [0, 4_000] {
        for statistic in [Aggregate::Median, Aggregate::Percentile(95), Aggregate::Percentile(99)] {
            let mut accumulator = Accumulator::with_settings(Agg::Of(statistic), StorageAccumulatorSettings { memory_bytes });
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(accumulator.push(0, &ScalarValue::Float(value)).is_err());
            }
            for (capture, value) in
                [ScalarValue::Null, ScalarValue::Missing, ScalarValue::Float(-f64::MAX), ScalarValue::Float(f64::MAX)].iter().enumerate()
            {
                accumulator.push(capture as u64, value).unwrap();
            }
            let branch = accumulator.clone();
            let mut calls = 0;
            let result = accumulator.finish(&mut || {
                calls += 1;
                if calls == 2 { Err(Error::Closing { domain: "cancel order statistics" }) } else { Ok(()) }
            });
            assert!(matches!(result, Err(Error::Closing { domain: "cancel order statistics" })));
            let ScalarValue::Float(value) = branch.finish(&mut || Ok(())).unwrap() else { panic!("expected float") };
            assert!(value.is_finite());
            if statistic == Aggregate::Median {
                assert_eq!(value, 0.0);
            }
        }
    }
}
