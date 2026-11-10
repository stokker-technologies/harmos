/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Cached And Spilled Coordinate Selection

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use arrow_array::{Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use harmos_storage::query::Coordinates;
use harmos_storage::query::transform::{
    FieldPath,
    downsample::{Algorithm, Downsample},
};
use std::sync::Arc;

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
fn cached_and_disk_coordinates_preserve_timestamps_and_bucket_selection() {
    use arrow_array::TimestampNanosecondArray;
    use arrow_schema::TimeUnit;
    // Build A Timestamped Series Near The Integer Limit.
    let schema = Arc::new(Schema::new(vec![
        Field::new("time", DataType::Timestamp(TimeUnit::Nanosecond, None), false),
        Field::new("value", DataType::Int64, false),
    ]));
    let rows = 9001;
    let timestamps = TimestampNanosecondArray::from((0..rows).map(|row| i64::MAX - 20_000 + row).collect::<Vec<_>>());
    let values = Int64Array::from((0..rows).map(|row| row * 7919 % 97).collect::<Vec<_>>());
    let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(timestamps), Arc::new(values)]).unwrap();
    let plan = Downsample {
        algorithm: Algorithm::Lttb,
        max_rows: 103,
        value: Some(FieldPath::from("value")),
        axis: Some(FieldPath::from("time")),
    };
    // Compare Cached And Disk Coordinates Across Page Boundaries.
    let mut cached = Coordinates::new(&batch, &plan, true).unwrap();
    let mut disk = Coordinates::new(&batch, &plan, false).unwrap();
    assert!(cached.is_cached());
    assert!(!disk.is_cached());
    for index in [0, 4095, 4096, 8192, 9000, 2, 5000] {
        assert_eq!(cached.get(index).unwrap(), disk.get(index).unwrap());
        assert_eq!(cached.get(index).unwrap().0, index as f64);
    }
    // Compare The Selected Buckets.
    let expected = cached.select(&plan).unwrap();
    let actual = disk.select(&plan).unwrap();
    assert_eq!(actual, expected);
}
