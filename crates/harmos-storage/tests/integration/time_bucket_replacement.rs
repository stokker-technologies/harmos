/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

// Third Party Imports
use arrow_array::{BinaryArray, Float64Array, Int64Array, ListArray, RecordBatch, UInt64Array};

// Local Imports
use crate::manager::backend;
use harmos_domain::{Message, schema::Described};
use harmos_storage::query::{
    TableWriter,
    columnar::{ArrowScan, Cancellation, ColumnKind, ColumnSource, list_column_rows},
    index::encode_key,
    projection::{ProjectionDefinition, ProjectionRead, ProjectionTransform, advance_projection},
    scalar::ScalarValue,
    time_buckets::TimeBucketPlan,
    transform::Transform,
};
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct Sample {
    identity: String,
    time_ns: i64,
    group: u64,
    value: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct Bucket {
    key: Vec<u8>,
    ordinal: i64,
    revision: Vec<u8>,
    count: u64,
    first: f64,
    last: f64,
    min: f64,
    max: f64,
    sum: f64,
    sum_squares: f64,
    mean: f64,
    m2: f64,
    first_position: u64,
    min_position: u64,
    max_position: u64,
    positions: Vec<u64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn key(group: u64, bucket: i64) -> Vec<u8> {
    encode_key(&[ScalarValue::Unsigned(group), ScalarValue::Signed(bucket)]).unwrap()
}
fn decode(batch: &RecordBatch) -> Vec<Bucket> {
    let float = |name: &str, row| batch.column_by_name(name).unwrap().as_any().downcast_ref::<Float64Array>().unwrap().value(row);
    let unsigned = |name: &str, row| batch.column_by_name(name).unwrap().as_any().downcast_ref::<UInt64Array>().unwrap().value(row);
    (0..batch.num_rows())
        .map(|row| {
            let positions = batch.column_by_name("positions").unwrap().as_any().downcast_ref::<ListArray>().unwrap().value(row);
            Bucket {
                key: batch.column_by_name("key").unwrap().as_any().downcast_ref::<BinaryArray>().unwrap().value(row).to_vec(),
                ordinal: batch.column_by_name("bucket").unwrap().as_any().downcast_ref::<Int64Array>().unwrap().value(row),
                revision: batch.column_by_name("revision").unwrap().as_any().downcast_ref::<BinaryArray>().unwrap().value(row).to_vec(),
                count: unsigned("count", row),
                first: float("first", row),
                last: float("last", row),
                min: float("min", row),
                max: float("max", row),
                sum: float("sum", row),
                sum_squares: float("sum_squares", row),
                mean: float("mean", row),
                m2: float("m2", row),
                first_position: unsigned("first_position", row),
                min_position: unsigned("min_position", row),
                max_position: unsigned("max_position", row),
                positions: positions.as_any().downcast_ref::<UInt64Array>().unwrap().values().to_vec(),
            }
        })
        .collect()
}
async fn read_all(backend: &dyn StorageBackend, mut read: ProjectionRead) -> (Vec<Bucket>, Vec<Vec<u8>>) {
    let mut buckets = Vec::new();
    let mut changed = Vec::new();
    loop {
        let page = backend.list_projection_rows(&read).await.unwrap();
        buckets.extend(decode(&page.batch));
        changed.extend(page.changed_keys);
        let Some(next) = page.next else {
            break;
        };
        read.after = next;
    }
    (buckets, changed)
}
fn record(history: &StoreHistory, schema: &SchemaKey, position: u64, identity: &str, time_ns: i64, group: u64, value: f64) -> StoredRecord {
    StoredRecord {
        position: StorageCursor {
            history: history.clone(),
            sequence: position,
        },
        schema: schema.clone(),
        scope: None,
        item: Identifier::new(format!("row-{position}")).unwrap(),
        source: None,
        source_time: None,
        payload: Sample {
            identity: identity.into(),
            time_ns,
            group,
            value,
        }
        .encode(),
    }
}

/* === Tests === */

#[tokio::test]
async fn late_samples_and_corrections_replace_grouped_buckets_at_saved_cuts() {
    // Capture Half-Open Boundaries, Ties And Two Independent Groups.
    let root = tempfile::tempdir().unwrap();
    let history = backend::history();
    let policy = backend::storage_settings();
    let native = SqliteBackend::new(
        root.path().join("buckets.sqlite"),
        history.clone(),
        SqliteSettings {
            chunk_rows: nz(3),
            chunk_bytes: policy.response.batch_bytes,
        },
    )
    .unwrap();
    let fallback = backend::Data::new(history.clone());
    let schema = SchemaKey {
        id: Identifier::new("sample").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph =
        harmos_domain::schema::graph::Schema::from_declaration(Sample::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap();
    let descriptor = StorageSchema::new(StorageCursor { history: history.clone(), sequence: 1 }, schema.clone(), graph);
    let records = vec![
        record(&history, &schema, 2, "a", -1, 0, 5.0),
        record(&history, &schema, 3, "b", 0, 0, 3.0),
        record(&history, &schema, 4, "c", 9, 0, 7.0),
        record(&history, &schema, 5, "d", 10, 0, 11.0),
        record(&history, &schema, 6, "e", 0, 1, 100.0),
        record(&history, &schema, 7, "f", 0, 0, 3.0),
    ];
    let plan = TimeBucketPlan {
        origin_ns: 0,
        width_ns: NonZeroU64::new(10).unwrap(),
        time: "payload.time_ns".into(),
        value: "payload.value".into(),
        identity: "payload.identity".into(),
        group: vec!["payload.group".into()],
        max_samples: nz(64),
        max_bytes: nz(1024 * 1024),
    };
    let definition = ProjectionDefinition {
        name: Identifier::new("buckets").unwrap(),
        version: 1,
        source: ColumnSource {
            kind: ColumnKind::Records,
            schema: schema.clone(),
        },
        transform: ProjectionTransform::TimeBuckets(plan.clone()),
    };
    let work = ReadLimits { rows: nz(64), bytes: nz(4 * 1024 * 1024) };
    let cancel = Cancellation::default();
    let cut = StorageCursor { history: history.clone(), sequence: 7 };
    let mut saved = Vec::new();
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.create_session().await.unwrap();
        backend.append_schemas(StorageBatch::new(vec![descriptor.clone()])).await.unwrap();
        backend.append_records(StorageBatch::new(records.clone())).await.unwrap();
        backend.set_projection_definition(&definition, false).await.unwrap();
        let snapshot = backend.create_snapshot(&cut).await.unwrap();
        advance_projection(backend, &snapshot, &definition.name, None, work, &cancel).await.unwrap();
        saved.push(snapshot);
    }
    let read = ProjectionRead {
        name: definition.name.clone(),
        revision: 1,
        frontier: cut.clone(),
        after: 0,
        limits: ReadLimits { rows: nz(1), bytes: nz(16384) },
        work,
    };
    let (initial, initial_keys) = read_all(&native, read.clone()).await;
    assert_eq!(initial, read_all(&fallback, read.clone()).await.0);
    assert_eq!(initial_keys, vec![key(0, -1), key(0, 0), key(0, 1), key(1, 0)]);
    let middle = &initial[1];
    assert_eq!(middle.count, 3);
    assert_eq!(middle.sum, 13.0);
    assert_eq!(middle.sum_squares, 67.0);
    assert_eq!(middle.positions, vec![3, 7, 4]);
    assert_eq!(middle.first_position, 3);
    assert_eq!(middle.min_position, 3);
    assert_eq!(initial[0].ordinal, -1);
    assert_eq!(initial[3].sum, 100.0);

    // Add A Late Sample And Move One Logical Identity Across A Bucket Boundary.
    let additions = vec![record(&history, &schema, 8, "late", 5, 0, -2.0), record(&history, &schema, 9, "c", 11, 0, 2.0)];
    let next = StorageCursor { history: history.clone(), sequence: 9 };
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.append_records(StorageBatch::new(additions.clone())).await.unwrap();
        let snapshot = backend.create_snapshot(&next).await.unwrap();
        let before = backend.get_work().await.unwrap();
        advance_projection(backend, &snapshot, &definition.name, None, work, &cancel).await.unwrap();
        let after = backend.get_work().await.unwrap();
        if backend.capabilities().native_arrow {
            assert_eq!(after.source_rows - before.source_rows, additions.len() as u64);
            assert_eq!(after.buckets - before.buckets, 2);
            assert!(after.bucket_rows - before.bucket_rows < records.len() as u64);
            assert!(after.bucket_steps - before.bucket_steps < (after.bucket_rows - before.bucket_rows) * 40 + 60);
        }
        backend.delete_snapshot(&snapshot).await.unwrap();
        assert_eq!(read_all(backend, read.clone()).await.0, initial);
        let (current, changed) = read_all(backend, ProjectionRead { frontier: next.clone(), ..read.clone() }).await;
        assert_eq!(current.len(), initial.len());
        assert_eq!(changed, vec![key(0, 0), key(0, 1)]);
        assert_eq!(current[1].count, 3);
        assert_eq!(current[1].sum, 4.0);
        assert_eq!(current[1].sum_squares, 22.0);
        assert_eq!(current[1].positions, vec![3, 7, 8]);
        assert_eq!(current[1].min_position, 8);
        assert_eq!(current[2].positions, vec![5, 9]);
        assert_eq!(current[2].sum, 13.0);
        assert_eq!(current[2].m2, 40.5);
        assert_ne!(current[1].revision, initial[1].revision);
        assert_eq!(current[0].revision, initial[0].revision);
        assert_eq!(current[3].revision, initial[3].revision);
    }

    // Remove A Now-Empty Bucket And Then Page More Changed Keys Than Output Rows.
    let moved = [
        record(&history, &schema, 10, "a", 12, 0, 5.0),
        record(&history, &schema, 11, "e", 12, 0, 100.0),
        record(&history, &schema, 12, "b", 13, 0, 3.0),
        record(&history, &schema, 13, "f", 14, 0, 3.0),
        record(&history, &schema, 14, "late", 15, 0, -2.0),
    ];
    let moved_cut = StorageCursor { history: history.clone(), sequence: 10 };
    let final_cut = StorageCursor { history: history.clone(), sequence: 14 };
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.append_records(StorageBatch::new(moved[..1].to_vec())).await.unwrap();
        let snapshot = backend.create_snapshot(&moved_cut).await.unwrap();
        advance_projection(backend, &snapshot, &definition.name, None, work, &cancel).await.unwrap();
        backend.delete_snapshot(&snapshot).await.unwrap();
        let (buckets, changed) = read_all(
            backend,
            ProjectionRead {
                frontier: moved_cut.clone(),
                ..read.clone()
            },
        )
        .await;
        assert_eq!(buckets.len(), 3);
        assert_eq!(changed, vec![key(0, -1), key(0, 1)]);
        backend.append_records(StorageBatch::new(moved[1..].to_vec())).await.unwrap();
        let snapshot = backend.create_snapshot(&final_cut).await.unwrap();
        advance_projection(backend, &snapshot, &definition.name, None, work, &cancel).await.unwrap();
        backend.delete_snapshot(&snapshot).await.unwrap();
        let (buckets, changed) = read_all(
            backend,
            ProjectionRead {
                frontier: final_cut.clone(),
                ..read.clone()
            },
        )
        .await;
        assert_eq!(buckets.len(), 1);
        assert_eq!(changed, vec![key(0, 0), key(0, 1), key(1, 0)]);
        assert_eq!(buckets[0].count, 7);
        assert_eq!(buckets[0].sum, 122.0);
    }

    // Apply The Native Transform To Replayed Paged Arrow Input.
    let snapshot = native.create_snapshot(&final_cut).await.unwrap();
    let mut scan = ArrowScan {
        source: definition.source.clone(),
        columns: vec![
            "_storage.position".into(),
            "payload.identity".into(),
            "payload.time_ns".into(),
            "payload.group".into(),
            "payload.value".into(),
        ],
        predicate: None,
        time: None,
        start: 0,
        after: None,
        limits: work,
        work,
    };
    let mut writer = None;
    loop {
        let page = list_column_rows(&native, &snapshot, &scan, &cancel).await.unwrap();
        let writer = writer.get_or_insert_with(|| TableWriter::new(page.batch.schema()).unwrap());
        writer.push(&page.batch, &mut || Ok(())).unwrap();
        writer.push(&page.batch, &mut || Ok(())).unwrap();
        if page.complete {
            break;
        }
        scan.after = page.next;
    }
    let table = writer.unwrap().finish().unwrap();
    let output = table.transform(&Transform::TimeBuckets(plan.clone()), &mut || Ok(())).unwrap();
    let mut output = output.reader().unwrap();
    let mut buckets = Vec::new();
    while let Some(page) = output.next(&mut || Ok(())).unwrap() {
        buckets.extend(decode(&page));
    }
    assert_eq!(
        buckets,
        read_all(
            &native,
            ProjectionRead {
                frontier: final_cut.clone(),
                ..read.clone()
            }
        )
        .await
        .0
    );
    let mut bounded = plan;
    bounded.max_samples = nz(1);
    assert!(table.transform(&Transform::TimeBuckets(bounded), &mut || Ok(())).is_err());

    // Refuse Non-Finite Input Without Advancing The Readable Frontier.
    let invalid = record(&history, &schema, 15, "invalid", 1, 0, f64::NAN);
    let invalid_cut = StorageCursor { history: history.clone(), sequence: 15 };
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.append_records(StorageBatch::new(vec![invalid.clone()])).await.unwrap();
        let snapshot = backend.create_snapshot(&invalid_cut).await.unwrap();
        assert!(advance_projection(backend, &snapshot, &definition.name, None, work, &cancel).await.is_err());
        assert_eq!(
            backend.get_projection_status(&definition.name, 1, None).await.unwrap().map(|status| status.frontier),
            Some(final_cut.clone())
        );
        backend.delete_snapshot(&snapshot).await.unwrap();
    }
    native.delete_snapshot(&snapshot).await.unwrap();
    // Refuse A Rebuilt Projection Before Native Or Fallback Advancement.
    let mut rebuilt = definition.clone();
    rebuilt.version += 1;
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        let revision = backend.set_projection_definition(&rebuilt, true).await.unwrap();
        let snapshot = backend.create_snapshot(&final_cut).await.unwrap();
        assert!(matches!(
            advance_projection(backend, &snapshot, &definition.name, Some(1), work, &cancel).await,
            Err(Error::Backend { kind: BackendFailure::Conflict, .. })
        ));
        assert_eq!(backend.get_projection_status(&definition.name, revision, None).await.unwrap().map(|status| status.frontier), None);
        advance_projection(backend, &snapshot, &definition.name, Some(revision), work, &cancel).await.unwrap();
        assert_eq!(
            backend.get_projection_status(&definition.name, revision, None).await.unwrap().map(|status| status.frontier),
            Some(final_cut.clone())
        );
        backend.delete_snapshot(&snapshot).await.unwrap();
    }
    native.delete_snapshot(&saved[0]).await.unwrap();
    native.delete_session().await.unwrap();
    fallback.delete_snapshot(&saved[1]).await.unwrap();
    fallback.delete_session().await.unwrap();
}
