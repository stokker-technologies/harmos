/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../support/storage.rs"]
#[allow(dead_code)]
mod backend;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::{Arc, atomic::Ordering},
};

// Third Party Imports
use arrow_array::{Float64Array, UInt64Array};

// Local Imports
use harmos_domain::{Message, schema::Described};
use harmos_storage::query::columnar::{ArrowScan, Cancellation, ColumnKind, ColumnPredicate, ColumnSource, TimeBounds, scan_arrow};
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct Sample {
    time_ns: i64,
    value: f64,
    unused: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */

#[tokio::test]
async fn saved_windows_match_fallback_while_live_writes_and_cancellation_advance() {
    // Open Both Backends And Declare One Stored Shape.
    let root = tempfile::tempdir().unwrap();
    let history = backend::history();
    let policy = backend::storage_settings();
    let native = SqliteBackend::new(
        root.path().join("storage.sqlite"),
        history.clone(),
        SqliteSettings {
            chunk_rows: nz(4),
            chunk_bytes: policy.response.batch_bytes,
        },
    )
    .unwrap();
    let fallback = Arc::new(backend::Data::new(history.clone()));
    native.open().await.unwrap();
    fallback.open().await.unwrap();
    let key = SchemaKey {
        id: Identifier::new("sample").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph =
        harmos_domain::schema::graph::Schema::from_declaration(Sample::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap();
    let schema = StorageSchema::new(StorageCursor { history: history.clone(), sequence: 1 }, key.clone(), graph);
    native.write_schemas(StorageBatch::new(vec![schema.clone()])).await.unwrap();
    fallback.write_schemas(StorageBatch::new(vec![schema])).await.unwrap();
    let records = (0..8)
        .map(|index| StoredRecord {
            position: StorageCursor {
                history: history.clone(),
                sequence: index + 2,
            },
            schema: key.clone(),
            scope: None,
            item: Identifier::new(format!("sample-{index}")).unwrap(),
            source: None,
            source_time: None,
            payload: Sample {
                time_ns: index as i64,
                value: index as f64,
                unused: vec![7; 1024],
            }
            .encode(),
        })
        .collect::<Vec<_>>();
    native.write_records(StorageBatch::new(records.clone())).await.unwrap();
    fallback.write_records(StorageBatch::new(records.clone())).await.unwrap();
    let cut = StorageCursor { history: history.clone(), sequence: 9 };
    let saved = native.open_snapshot(&cut).await.unwrap();
    let saved_fallback = fallback.open_snapshot(&cut).await.unwrap();

    // Save A Later Sample Without Changing Either Retained Cut.
    let mut late = records[4].clone();
    late.position.sequence = 10;
    native.write_records(StorageBatch::new(vec![late.clone()])).await.unwrap();
    fallback.write_records(StorageBatch::new(vec![late])).await.unwrap();
    let read = ArrowScan {
        source: ColumnSource { kind: ColumnKind::Records, schema: key },
        columns: vec!["_storage.position".into(), "payload.value".into()],
        predicate: Some(ColumnPredicate {
            column: "payload.value".into(),
            lower: Some(3.0),
            upper: Some(6.0),
        }),
        time: Some(TimeBounds {
            column: "payload.time_ns".into(),
            lower_ns: 2,
            upper_ns: 7,
        }),
        start: 0,
        after: None,
        limits: ReadLimits { rows: nz(2), bytes: nz(4096) },
        work: ReadLimits {
            rows: nz(4),
            bytes: policy.response.batch_bytes,
        },
    };
    for (backend, snapshot, native_path) in
        [(&native as &dyn StorageBackend, &saved, true), (fallback.as_ref() as &dyn StorageBackend, &saved_fallback, false)]
    {
        let mut read = read.clone();
        let mut values = Vec::new();
        let mut positions = Vec::new();
        loop {
            let page = scan_arrow(backend, snapshot, &read, &Cancellation::default()).await.unwrap();
            values.extend_from_slice(page.batch.column(1).as_any().downcast_ref::<Float64Array>().unwrap().values());
            positions.extend_from_slice(page.batch.column(0).as_any().downcast_ref::<UInt64Array>().unwrap().values());
            if native_path {
                assert_eq!(page.payload_rows, 0);
                assert!(!page.columns_read.iter().any(|name| name == "payload.unused"));
            }
            if page.complete {
                break;
            }
            let next = page.next.unwrap();
            assert!(next.position > read.after.as_ref().map_or(read.start, |after| after.position));
            read.after = Some(next);
        }
        assert_eq!(values, vec![3.0, 4.0, 5.0]);
        assert_eq!(positions, vec![5, 6, 7]);
    }

    // Prune Whole Time Chunks Before Reading Any Column Bytes.
    let mut pruned = read.clone();
    pruned.predicate = None;
    pruned.time = Some(TimeBounds {
        column: "payload.time_ns".into(),
        lower_ns: 100,
        upper_ns: 101,
    });
    loop {
        let page = scan_arrow(&native, &saved, &pruned, &Cancellation::default()).await.unwrap();
        assert_eq!(page.batch.num_rows(), 0);
        assert_eq!(page.bytes_read, 0);
        if page.complete {
            break;
        }
        pruned.after = page.next;
    }

    // Reject A Changed Plan And Cancel An Already Entered Fallback Read.
    let page = scan_arrow(&native, &saved, &read, &Cancellation::default()).await.unwrap();
    let mut changed = read.clone();
    changed.after = page.next;
    changed.columns.reverse();
    assert!(scan_arrow(&native, &saved, &changed, &Cancellation::default()).await.is_err());
    fallback.control.block_read.store(true, Ordering::SeqCst);
    let cancelled = Cancellation::default();
    let pending = {
        let fallback = fallback.clone();
        let snapshot = saved_fallback.clone();
        let read = read.clone();
        let cancel = cancelled.clone();
        tokio::spawn(async move { scan_arrow(fallback.as_ref(), &snapshot, &read, &cancel).await })
    };
    fallback.control.read_entered.notified().await;
    cancelled.cancel();
    fallback.control.read_release.notify_one();
    assert!(pending.await.unwrap().is_err());
    fallback.control.block_read.store(false, Ordering::SeqCst);

    // Refuse Released And Restarted Snapshot Tokens.
    native.release_snapshot(&saved).await.unwrap();
    assert!(scan_arrow(&native, &saved, &read, &Cancellation::default()).await.is_err());
    native.close().await.unwrap();
    drop(native);
    let reopened = SqliteBackend::new(
        root.path().join("storage.sqlite"),
        history,
        SqliteSettings {
            chunk_rows: nz(4),
            chunk_bytes: policy.response.batch_bytes,
        },
    )
    .unwrap();
    reopened.open().await.unwrap();
    assert!(scan_arrow(&reopened, &saved, &read, &Cancellation::default()).await.is_err());
    let current = reopened.open_snapshot(&cut).await.unwrap();
    assert_ne!(current.token, saved.token);
    reopened.release_snapshot(&current).await.unwrap();
    reopened.close().await.unwrap();
    fallback.release_snapshot(&saved_fallback).await.unwrap();
    fallback.close().await.unwrap();
}
