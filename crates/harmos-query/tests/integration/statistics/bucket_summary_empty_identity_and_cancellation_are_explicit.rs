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
use arrow_array::UInt64Array;

// Local Imports
use super::{buckets, input};
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
fn bucket_summary_empty_identity_and_cancellation_are_explicit() {
    let batch = input(vec![], 1);
    let transform = Transform::BucketSummary(buckets(0, 2));
    let empty = transform.apply(&batch, &mut || Ok(())).unwrap();
    assert_eq!(empty.num_rows(), 1);
    assert_eq!(empty.column_by_name("count").unwrap().as_any().downcast_ref::<UInt64Array>().unwrap().value(0), 0);
    assert!(empty.column_by_name("mean").unwrap().is_null(0));
    assert!(!empty.column_by_name("continuation").unwrap().is_null(0));
    let batch = input(vec![1.0; 32], 1);
    for transform in [
        Transform::BucketSummary(buckets(32, 2)),
        Transform::BoxSummary(BoxSummary {
            field: "value".into(),
            max_outliers: 0,
            memory_bytes: 0,
            scratch_bytes: 65536,
        }),
    ] {
        let mut calls = 0;
        let result = transform.apply(&batch, &mut || {
            calls += 1;
            if calls == 12 { Err(harmos_query::Error::Closing { domain: "statistics cancellation" }) } else { Ok(()) }
        });
        assert!(matches!(result, Err(harmos_query::Error::Closing { domain: "statistics cancellation" })));
    }
}
