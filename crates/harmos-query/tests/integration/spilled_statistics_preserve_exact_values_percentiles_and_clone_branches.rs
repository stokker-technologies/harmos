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

// Third Party Imports

// Local Imports
use harmos_query::config::QueryAccumulatorSettings;
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

/* === Tests === */

#[test]
fn spilled_statistics_preserve_exact_values_percentiles_and_clone_branches() {
    let settings = QueryAccumulatorSettings { memory_bytes: 0 };
    let mut median = Accumulator::with_settings(Agg::Of(Aggregate::Median), settings);
    median.push(0, &ScalarValue::Unsigned(9_007_199_254_740_993)).unwrap();
    median.push(1, &ScalarValue::Unsigned(9_007_199_254_740_995)).unwrap();
    let mut branch = median.clone();
    median.push(2, &ScalarValue::Unsigned(9_007_199_254_740_997)).unwrap();
    branch.push(2, &ScalarValue::Unsigned(9_007_199_254_740_991)).unwrap();
    assert_eq!(median.finish(&mut || Ok(())).unwrap(), ScalarValue::Unsigned(9_007_199_254_740_995));
    assert_eq!(branch.finish(&mut || Ok(())).unwrap(), ScalarValue::Unsigned(9_007_199_254_740_993));
    let values = vec![-7.0, 3.0, 1.0, -0.0, 9.0, -2.0, 3.0, 0.0];
    let mut sorted = values.clone();
    sorted.sort_by(f64::total_cmp);
    for rank in [0, 13, 50, 77, 100] {
        let mut accumulator = Accumulator::with_settings(Agg::Of(Aggregate::Percentile(rank)), settings);
        for (index, value) in values.iter().enumerate() {
            accumulator.push(index as u64, &ScalarValue::Float(*value)).unwrap();
        }
        assert_eq!(
            accumulator.finish(&mut || Ok(())).unwrap(),
            ScalarValue::Float({
                let at = f64::from(rank) / 100.0 * (sorted.len() - 1) as f64;
                let low = at.floor() as usize;
                let high = at.ceil() as usize;
                sorted[low] + (sorted[high] - sorted[low]) * at.fract()
            })
        );
    }
}

#[test]
fn one_accumulation_returns_typed_median_and_all_float_percentiles() {
    let statistics = [
        Aggregate::Median,
        Aggregate::Percentile(0),
        Aggregate::Percentile(5),
        Aggregate::Percentile(50),
        Aggregate::Percentile(95),
        Aggregate::Percentile(99),
        Aggregate::Percentile(100),
        Aggregate::Median,
    ];
    for memory_bytes in [0, 160, 4096, 65536] {
        let settings = QueryAccumulatorSettings { memory_bytes };
        for signed in [false, true] {
            let mut accumulator = Accumulator::with_settings(Agg::Of(Aggregate::Median), settings);
            let mut separate = statistics.map(|statistic| Accumulator::with_settings(Agg::Of(statistic), settings));
            for index in 0..67 {
                let number = 9_007_199_254_740_993 + (index * 31 % 67);
                let value = if signed { ScalarValue::Signed(number as i64) } else { ScalarValue::Unsigned(number) };
                accumulator.push(index, &value).unwrap();
                for single in &mut separate {
                    single.push(index, &value).unwrap();
                }
            }
            accumulator.push(67, &ScalarValue::Null).unwrap();
            accumulator.push(68, &ScalarValue::Missing).unwrap();
            let mut branch = accumulator.clone();
            let expected = separate.into_iter().map(|single| single.finish(&mut || Ok(())).unwrap()).collect::<Vec<_>>();
            let shared = accumulator.finish_ranks(&statistics, &mut || Ok(())).unwrap();
            assert_eq!(shared, expected);
            assert!(matches!(&shared[0], ScalarValue::Signed(_) | ScalarValue::Unsigned(_)));
            assert!(shared[1..7].iter().all(|value| matches!(value, ScalarValue::Float(_))));
            branch.push(69, &ScalarValue::Unsigned(0)).unwrap();
            let values = branch.finish_ranks(&[Aggregate::Median], &mut || Ok(())).unwrap();
            assert_ne!(values[0], shared[0]);
        }
    }
    let mut singleton = Accumulator::with_settings(Agg::Of(Aggregate::Median), QueryAccumulatorSettings { memory_bytes: 0 });
    singleton.push(0, &ScalarValue::Unsigned(9_007_199_254_740_993)).unwrap();
    let values = singleton.finish_ranks(&[Aggregate::Median, Aggregate::Percentile(95)], &mut || Ok(())).unwrap();
    assert_eq!(values, vec![ScalarValue::Unsigned(9_007_199_254_740_993), ScalarValue::Float(9_007_199_254_740_992.0)]);
    let empty = Accumulator::new(Agg::Of(Aggregate::Median));
    assert_eq!(empty.finish_ranks(&statistics, &mut || Ok(())).unwrap(), vec![ScalarValue::Missing; statistics.len()]);
    assert!(Accumulator::new(Agg::Of(Aggregate::Median)).finish_ranks(&[Aggregate::Percentile(101)], &mut || Ok(())).is_err());
}
