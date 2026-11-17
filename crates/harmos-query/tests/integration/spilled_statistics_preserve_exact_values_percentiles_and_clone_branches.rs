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
