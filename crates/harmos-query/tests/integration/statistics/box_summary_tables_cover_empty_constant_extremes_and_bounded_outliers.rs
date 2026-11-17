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
use arrow_array::{Array, Float64Array, ListArray, UInt64Array};

// Local Imports
use super::{close, input};
use harmos_domain::model::storage::query::transform::statistics::BoxSummary;
use harmos_query::transform::{Transform, TransformExecution};

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
fn box_summary_tables_cover_empty_constant_extremes_and_bounded_outliers() {
    let cases = [
        (vec![], [None, None, None], [None, None], 0, vec![]),
        (vec![7.0; 5], [Some(7.0); 3], [Some(7.0); 2], 0, vec![]),
        (
            vec![-f64::MAX, -f64::MAX, -f64::MAX / 2.0, -f64::MAX / 4.0, f64::MAX],
            [Some(-f64::MAX), Some(-f64::MAX / 2.0), Some(-f64::MAX / 4.0)],
            [Some(-f64::MAX), Some(-f64::MAX / 4.0)],
            1,
            vec![f64::MAX],
        ),
        (vec![-f64::MAX, f64::MAX], [Some(-f64::MAX / 2.0), Some(0.0), Some(f64::MAX / 2.0)], [Some(-f64::MAX), Some(f64::MAX)], 0, vec![]),
        (vec![100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -100.0], [Some(0.0); 3], [Some(0.0); 2], 2, vec![100.0]),
    ];
    for (values, quartiles, whiskers, count, outliers) in cases {
        let batch = input(values, 1);
        for memory_bytes in [0, 4096] {
            let plan = Transform::BoxSummary(BoxSummary {
                field: "value".into(),
                max_outliers: 1,
                memory_bytes,
                scratch_bytes: 65536,
            });
            let output = plan.apply(&batch, &mut || Ok(())).unwrap();
            for (column, expected) in quartiles.into_iter().chain(whiskers).enumerate() {
                let actual = output.column(column).as_any().downcast_ref::<Float64Array>().unwrap();
                assert_eq!(actual.is_null(0), expected.is_none());
                if let Some(expected) = expected {
                    close(actual.value(0), expected);
                }
            }
            assert_eq!(output.column(5).as_any().downcast_ref::<UInt64Array>().unwrap().value(0), count);
            let actual = output.column(6).as_any().downcast_ref::<ListArray>().unwrap().value(0);
            assert_eq!(actual.as_any().downcast_ref::<Float64Array>().unwrap().values().as_ref(), &outliers);
        }
    }
}
