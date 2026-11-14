/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::manager::{backend, fixture};
use harmos_query::columnar::ArrowScan;
use harmos_query::columnar::Cancellation;
use harmos_query::columnar::ColumnKind;
use harmos_query::columnar::ColumnSource;
use harmos_storage::*;
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};

/* === Structs === */

#[derive(harmos_domain::Message, Clone)]
struct Payload {
    bytes: Vec<u8>,
}

/* === Tests === */

#[tokio::test]
async fn coalesced_admissions_keep_physical_chunks_bounded_and_commit_atomically() {
    let root = tempfile::tempdir().unwrap();
    let history = backend::history();
    let settings = backend::storage_settings();
    let physical_bytes = NonZeroUsize::new(16 * 1024).unwrap();
    let primary = Arc::new(
        sqlite::SqliteBackend::new(
            root.path().join("data.sqlite"),
            history.clone(),
            sqlite::SqliteSettings {
                chunk_rows: NonZeroUsize::new(256).unwrap(),
                chunk_bytes: physical_bytes,
            },
        )
        .unwrap(),
    );
    let storage = StorageManager::new(
        StorageBackends {
            primary: primary.clone(),
            state: Arc::new(backend::Recovery::new(history)),
        },
        settings,
    )
    .unwrap();
    fixture::start(&storage).await;
    let key = SchemaKey {
        id: Identifier::new("payload").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = schema::graph::Schema::from_declaration(
        <Payload as schema::Described>::DEFINITION,
        &harmos_domain::config::SchemaSettings::default(),
    )
    .unwrap();
    storage.write_schema(key.clone(), graph).await.unwrap();
    storage.flush().await.unwrap();
    let before = primary.get_work().await.unwrap();
    let rows = (0..65)
        .map(|index| StreamInput {
            record: RecordInput {
                schema: key.clone(),
                scope: None,
                item: Identifier::new("payload").unwrap(),
                source: None,
                source_time: None,
                payload: Payload {
                    bytes: vec![index as u8; if index < 64 { 1024 } else { 32 }],
                }
                .encode(),
            },
            stream: Identifier::new("payload").unwrap(),
            sequence: index + 1,
        })
        .collect();
    let accepted = storage.append_samples(rows).await.unwrap();
    storage.flush().await.unwrap();
    let after = primary.get_work().await.unwrap();
    assert_eq!(after.synchronous_commits - before.synchronous_commits, 1);
    let read = storage.snapshot(&accepted).await.unwrap();
    let mut scan = ArrowScan {
        source: ColumnSource { kind: ColumnKind::Stream, schema: key },
        columns: vec!["_storage.position".into(), "payload.bytes".into()],
        predicate: None,
        time: None,
        start: 0,
        after: None,
        limits: ReadLimits {
            rows: NonZeroUsize::new(256).unwrap(),
            bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        },
        work: ReadLimits {
            rows: NonZeroUsize::new(256).unwrap(),
            bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        },
    };
    let mut positions = Vec::new();
    let mut pages = 0;
    loop {
        let page = read.list_column_rows(&scan, &Cancellation::default()).await.unwrap();
        assert!(page.bytes_read <= physical_bytes.get());
        let column = page.batch.column(0).as_any().downcast_ref::<arrow_array::UInt64Array>().unwrap();
        positions.extend_from_slice(column.values());
        pages += 1;
        if page.complete {
            break;
        }
        scan.after = page.next;
    }
    assert!(pages > 1);
    assert_eq!(positions, (2..=66).collect::<Vec<_>>());
    drop(read);
    storage.close().await.unwrap();
    drop(storage);
    drop(primary);

    // A Later Oversized Physical Row Rolls Back Earlier Chunks In The Same Commit.
    let native = sqlite::SqliteBackend::new(
        root.path().join("data.sqlite"),
        backend::history(),
        sqlite::SqliteSettings {
            chunk_rows: NonZeroUsize::new(256).unwrap(),
            chunk_bytes: physical_bytes,
        },
    )
    .unwrap();
    native.create_session().await.unwrap();
    let stored = |sequence, bytes| StoredRow {
        record: StoredRecord {
            position: StorageCursor { history: backend::history(), sequence },
            schema: scan.source.schema.clone(),
            scope: None,
            item: Identifier::new("payload").unwrap(),
            source: None,
            source_time: None,
            payload: Payload { bytes: vec![7; bytes] }.encode(),
        },
        stream: Identifier::new("payload").unwrap(),
        sequence: sequence - 1,
    };
    assert!(native.append_samples(StorageBatch::new(vec![stored(67, 32), stored(68, physical_bytes.get() * 2)])).await.is_err());
    assert_eq!(native.get_saved_position().await.unwrap(), accepted);
    let saved = native.append_samples(StorageBatch::new(vec![stored(67, 64)])).await.unwrap();
    assert_eq!(saved.sequence, 67);
    native.delete_session().await.unwrap();
}
