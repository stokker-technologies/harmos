/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exact Numeric Spool Parity Across Its Memory Boundary

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_storage::query::quantiles::Quantiles;

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
fn exact_quantile_spill() {
    for bytes in [0, 1024] {
        let mut values = Quantiles::new(bytes);
        for value in [3.0, 10.0, 0.0, f64::from_bits(1), -10.0, -0.0, 3.0] {
            values.push(value).unwrap();
        }
        let result = values.values(&[0, 25, 50, 75, 100]).unwrap();
        for (percentile, expected) in [(0, -10.0f64), (25, -0.0), (50, f64::from_bits(1)), (75, 3.0), (100, 10.0)] {
            assert_eq!(result[&percentile].to_bits(), expected.to_bits());
        }
        assert_eq!(values.spilled(), bytes == 0);
    }
    let mut extreme = Quantiles::new(0);
    extreme.push(-f64::MAX).unwrap();
    extreme.push(f64::MAX).unwrap();
    assert_eq!(extreme.values(&[50]).unwrap()[&50], 0.0);
}
