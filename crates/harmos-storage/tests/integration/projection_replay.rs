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
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::atomic::Ordering,
};

// Third Party Imports
use arrow_array::Float64Array;

// Local Imports
use crate::manager::backend;
use harmos_domain::{Message, schema::Described};
use harmos_storage::query::columnar::{Cancellation, ColumnKind, ColumnSource};
use harmos_storage::query::projection::{ProjectionDefinition, ProjectionRead, ProjectionTransform, advance_projection};
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct Sample {
    value: f64,
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
async fn projection_frontiers_survive_interrupted_publication_rebuild_and_restart() {
    // Open Exclusive Storage And Register A Versioned Projection.
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("projection.sqlite");
    let history = backend::history();
    let policy = backend::storage_settings();
    let settings = SqliteSettings {
        chunk_rows: nz(2),
        chunk_bytes: policy.response.batch_bytes,
    };
    let native = SqliteBackend::new(&path, history.clone(), settings).unwrap();
    assert!(SqliteBackend::new(&path, history.clone(), settings).is_err());
    let fallback = backend::Data::new(history.clone());
    let key = SchemaKey {
        id: Identifier::new("sample").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph =
        harmos_domain::schema::graph::Schema::from_declaration(Sample::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap();
    let descriptor = StorageSchema::new(StorageCursor { history: history.clone(), sequence: 1 }, key.clone(), graph);
    let records = (0..5)
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
            payload: Sample { value: index as f64 }.encode(),
        })
        .collect::<Vec<_>>();
    let definition = ProjectionDefinition {
        name: Identifier::new("values").unwrap(),
        version: 1,
        source: ColumnSource { kind: ColumnKind::Records, schema: key },
        transform: ProjectionTransform::Select(vec!["payload.value".into()]),
    };
    let work = ReadLimits { rows: nz(32), bytes: nz(1024 * 1024) };
    let cancel = Cancellation::default();
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.open().await.unwrap();
        backend.write_schemas(StorageBatch::new(vec![descriptor.clone()])).await.unwrap();
        backend.write_records(StorageBatch::new(records[..2].to_vec())).await.unwrap();
        assert_eq!(
            backend.register_projection(&definition, false).await.unwrap(),
            backend.register_projection(&definition, false).await.unwrap()
        );
    }
    let first_cut = StorageCursor { history: history.clone(), sequence: 3 };
    let first_native = native.open_snapshot(&first_cut).await.unwrap();
    let first_fallback = fallback.open_snapshot(&first_cut).await.unwrap();
    let revision = advance_projection(&native, &first_native, &definition.name, work, &cancel).await.unwrap();
    advance_projection(&fallback, &first_fallback, &definition.name, work, &cancel).await.unwrap();
    let old_read = ProjectionRead {
        name: definition.name.clone(),
        revision,
        frontier: first_cut.clone(),
        after: 0,
        limits: work,
        work,
    };

    // Inject A Failure After The First Output Page Has Been Written In The Transaction.
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.write_records(StorageBatch::new(records[2..].to_vec())).await.unwrap();
    }
    let next_cut = StorageCursor { history: history.clone(), sequence: 6 };
    let next_native = native.open_snapshot(&next_cut).await.unwrap();
    let next_fallback = fallback.open_snapshot(&next_cut).await.unwrap();
    let fault = rusqlite::Connection::open(&path).unwrap();
    fault.execute_batch("CREATE TRIGGER interrupt_projection BEFORE INSERT ON projection_pages WHEN NEW.first_row>0 BEGIN SELECT RAISE(ABORT,'injected projection interruption'); END;").unwrap();
    assert!(advance_projection(&native, &next_native, &definition.name, work, &cancel).await.is_err());
    fallback.control.fail_projection.store(true, Ordering::SeqCst);
    assert!(advance_projection(&fallback, &next_fallback, &definition.name, work, &cancel).await.is_err());
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        assert!(backend.projection_status(&definition.name, revision, &next_cut).await.unwrap().is_none());
        assert_eq!(backend.projection_frontier(&definition.name, revision).await.unwrap(), Some(first_cut.clone()));
    }
    let partial: i64 = fault.query_row("SELECT count(*) FROM projection_pages WHERE frontier=6", [], |row| row.get(0)).unwrap();
    assert_eq!(partial, 0);
    native.close().await.unwrap();
    drop(native);

    // Reopen, Keep The Published Cut, And Replay The Interrupted Candidate Once.
    let native = SqliteBackend::new(&path, history.clone(), settings).unwrap();
    native.open().await.unwrap();
    let old = native.read_projection(&old_read).await.unwrap();
    assert_eq!(old.batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap().values().as_ref(), &[0.0, 1.0]);
    fault.execute_batch("DROP TRIGGER interrupt_projection;").unwrap();
    fallback.control.fail_projection.store(false, Ordering::SeqCst);
    let fresh = native.open_snapshot(&next_cut).await.unwrap();
    assert_eq!(advance_projection(&native, &fresh, &definition.name, work, &cancel).await.unwrap(), revision);
    assert_eq!(advance_projection(&native, &fresh, &definition.name, work, &cancel).await.unwrap(), revision);
    advance_projection(&fallback, &next_fallback, &definition.name, work, &cancel).await.unwrap();
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        assert_eq!(backend.projection_status(&definition.name, revision, &next_cut).await.unwrap().unwrap().rows, records.len());
        let mut read = ProjectionRead {
            frontier: next_cut.clone(),
            limits: ReadLimits { rows: nz(1), bytes: nz(1024) },
            ..old_read.clone()
        };
        let mut values = vec![];
        loop {
            let page = backend.read_projection(&read).await.unwrap();
            values.extend_from_slice(page.batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap().values());
            let Some(next) = page.next else {
                break;
            };
            read.after = next;
        }
        assert_eq!(values, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
        let mut lag = read.clone();
        lag.frontier.sequence += 1;
        lag.after = 0;
        assert!(matches!(backend.read_projection(&lag).await, Err(Error::NotReady { .. })));
        assert_eq!(backend.read_projection(&old_read).await.unwrap().batch.num_rows(), 2);
    }

    // Rebuild Explicitly, Preserve Prior Revisions, Then Retire Only The Chosen Cut.
    let mut rebuilt = definition.clone();
    rebuilt.version += 1;
    rebuilt.transform = ProjectionTransform::Select(vec!["_storage.position".into(), "payload.value".into()]);
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        assert!(backend.register_projection(&rebuilt, false).await.is_err());
        let next_revision = backend.register_projection(&rebuilt, true).await.unwrap();
        assert_ne!(next_revision, revision);
        assert_eq!(backend.read_projection(&old_read).await.unwrap().batch.num_rows(), 2);
        backend.retire_projection(&definition.name, revision, &first_cut).await.unwrap();
        assert!(matches!(backend.read_projection(&old_read).await, Err(Error::Gap { .. })));
        let preserved = ProjectionRead {
            frontier: next_cut.clone(),
            ..old_read.clone()
        };
        assert!(backend.read_projection(&preserved).await.is_ok());
    }
    native.release_snapshot(&fresh).await.unwrap();
    native.close().await.unwrap();
    fallback.release_snapshot(&first_fallback).await.unwrap();
    fallback.release_snapshot(&next_fallback).await.unwrap();
    fallback.close().await.unwrap();
}
