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
        assert_eq!(values.spilled(), bytes == 0);
        let result = values.values(&[0, 25, 50, 75, 100], 65536, &mut || Ok(())).unwrap();
        for (percentile, expected) in [(0, -10.0f64), (25, -0.0), (50, f64::from_bits(1)), (75, 3.0), (100, 10.0)] {
            assert_eq!(result[&percentile].to_bits(), expected.to_bits());
        }
    }
    let mut extreme = Quantiles::new(0);
    extreme.push(-f64::MAX).unwrap();
    extreme.push(f64::MAX).unwrap();
    assert_eq!(extreme.values(&[50], 65536, &mut || Ok(())).unwrap()[&50], 0.0);

    for memory in [0, 16384, 32768] {
        let mut values = Quantiles::new(memory);
        assert!(values.push(f64::NAN).is_err());
        assert!(values.push(f64::INFINITY).is_err());
        for value in 0..2048 {
            values.push(value as f64).unwrap();
            assert!(values.size() <= memory + std::mem::size_of::<Quantiles>());
        }
        let mut checks = 0;
        let result = values.values(&[50], 65536, &mut || {
            checks += 1;
            if checks == 4 {
                Err(harmos_storage::Error::InvalidInput {
                    field: "cancelled",
                    reason: "test cancellation",
                })
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(harmos_storage::Error::InvalidInput { field: "cancelled", .. })));
    }
    let mut values = Quantiles::new(0);
    values.push(1.0).unwrap();
    assert!(values.values(&[50], 0, &mut || Ok(())).is_err());
    assert!(Quantiles::new(0).values(&[101], 65536, &mut || Ok(())).is_err());

    let mut sorted: Vec<_> = (0..4096).map(|index| ((index * 7919) % 4093) as f64 - 2046.0).collect();
    sorted.extend([-0.0, 0.0, f64::from_bits(1), -f64::MAX, f64::MAX]);
    for memory in [0, sorted.len() * 16] {
        let mut spool = Quantiles::new(memory);
        for value in &sorted {
            spool.push(*value).unwrap();
        }
        assert_eq!(spool.spilled(), memory == 0);
        let percentiles: Vec<_> = (0..=100).collect();
        let result = spool.values(&percentiles, 1024 * 1024, &mut || Ok(())).unwrap();
        sorted.sort_by(f64::total_cmp);
        for percentile in percentiles {
            let numerator = (sorted.len() - 1) * usize::from(percentile);
            let lower = numerator / 100;
            let upper = lower + usize::from(!numerator.is_multiple_of(100));
            let (low, high) = (sorted[lower], sorted[upper]);
            let fraction = (numerator % 100) as f64 / 100.0;
            let expected = if low == high {
                low
            } else if low.signum() == high.signum() {
                low + (high - low) * fraction
            } else {
                low * (1.0 - fraction) + high * fraction
            };
            assert_eq!(result[&percentile].to_bits(), expected.to_bits());
        }
    }
}
