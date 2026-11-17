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
use harmos_query::Quantiles;

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
fn several_percentiles_share_the_single_selection_passes() {
    for memory in [0, 32768] {
        let select = |ranks: &[u8]| {
            let mut spool = Quantiles::new(memory);
            for index in 0..1025 {
                spool.push(f64::from((index * 31) % 1025)).unwrap();
            }
            let mut visits = 0;
            let values = spool
                .values(ranks, 1024 * 1024, &mut || {
                    visits += 1;
                    Ok(())
                })
                .unwrap();
            (values, visits)
        };
        let (shared, shared_passes) = select(&[0, 13, 25, 50, 75, 95, 100]);
        for rank in [0, 13, 25, 50, 75, 95, 100] {
            let (single, single_passes) = select(&[rank]);
            assert_eq!(shared[&rank], single[&rank]);
            assert_eq!(shared_passes, single_passes);
        }
    }
}
