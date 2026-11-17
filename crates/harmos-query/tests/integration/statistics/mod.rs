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

mod box_summary_tables_cover_empty_constant_extremes_and_bounded_outliers;
mod bucket_continuation_equals_whole_range_across_pages_and_bounds;
mod bucket_summary_empty_identity_and_cancellation_are_explicit;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Arc;

// Third Party Imports
use arrow_array::{Array, Float64Array, Int64Array, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema};

// Local Imports
use harmos_domain::Identifier;
use harmos_domain::model::storage::query::transform::statistics::BucketSummary;
use harmos_domain::model::storage::{StorageCursor, StoreHistory};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn input(values: Vec<f64>, start: u64) -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("time", DataType::Int64, false),
        Field::new("position", DataType::UInt64, false),
        Field::new("value", DataType::Float64, false),
    ]));
    let positions = (start..start + values.len() as u64).collect::<Vec<_>>();
    let times = positions.iter().map(|position| (*position as i64 * 17) % 61 - 30).collect::<Vec<_>>();
    RecordBatch::try_new(
        schema,
        vec![Arc::new(Int64Array::from(times)), Arc::new(UInt64Array::from(positions)), Arc::new(Float64Array::from(values))],
    )
    .unwrap()
}
fn cut(sequence: u64) -> StorageCursor {
    StorageCursor {
        history: StoreHistory {
            store: Identifier::new("statistics").unwrap(),
            epoch: Identifier::new("test").unwrap(),
        },
        sequence,
    }
}
fn buckets(sequence: u64, max_buckets: usize) -> BucketSummary {
    BucketSummary {
        time: "time".into(),
        position: "position".into(),
        value: "value".into(),
        width_ns: NonZeroU64::new(1).unwrap(),
        max_buckets: NonZeroUsize::new(max_buckets).unwrap(),
        source_cut: cut(sequence),
        prior: None,
    }
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() <= 1e-10 * expected.abs().max(1.0), "{actual} != {expected}");
}
fn equal_envelopes(left: &RecordBatch, right: &RecordBatch) {
    assert_eq!(left.num_rows(), right.num_rows());
    for (field, (left, right)) in left.schema().fields().iter().zip(left.columns().iter().zip(right.columns())) {
        if field.name() == "continuation" {
            continue;
        }
        if let (Some(left), Some(right)) = (left.as_any().downcast_ref::<Float64Array>(), right.as_any().downcast_ref::<Float64Array>()) {
            for row in 0..left.len() {
                assert_eq!(left.is_null(row), right.is_null(row));
                if !left.is_null(row) {
                    close(left.value(row), right.value(row));
                }
            }
        } else {
            assert_eq!(left, right);
        }
    }
}

/* === Tests === */
