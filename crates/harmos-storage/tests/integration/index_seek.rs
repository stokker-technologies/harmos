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
use std::num::{NonZeroU32, NonZeroUsize};

// Third Party Imports

// Local Imports
use crate::manager::backend;
use harmos_domain::{Message, schema::Described};
use harmos_query::columnar::Cancellation;
use harmos_query::columnar::ColumnKind;
use harmos_query::columnar::ColumnSource;
use harmos_query::index::IndexDefinition;
use harmos_query::index::IndexRead;
use harmos_query::index::encode_key;
use harmos_storage::query::index::list_index_entries;
use harmos_storage::query::scalar::ScalarValue;
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct Sample {
    key: u64,
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
async fn seek_pages_keep_ties_and_saved_cuts_through_rebuild_and_restart() {
    // Declare A Source And Register A Durable And A Fallback Index.
    let root = tempfile::tempdir().unwrap();
    let history = backend::history();
    let policy = backend::storage_settings();
    let settings = SqliteSettings {
        chunk_rows: nz(4),
        chunk_bytes: policy.response.batch_bytes,
    };
    let native = SqliteBackend::new(root.path().join("index.sqlite"), history.clone(), settings).unwrap();
    let fallback = backend::Backend::new(history.clone());
    let schema = SchemaKey {
        id: Identifier::new("sample").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph =
        harmos_domain::schema::graph::Schema::from_declaration(Sample::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap();
    let descriptor = StorageSchema::new(StorageCursor { history: history.clone(), sequence: 1 }, schema.clone(), graph);
    let records = [2, 1, 1, 3, 1, 2, 2, 4]
        .into_iter()
        .enumerate()
        .map(|(index, key)| StoredRecord {
            position: StorageCursor {
                history: history.clone(),
                sequence: index as u64 + 2,
            },
            schema: schema.clone(),
            scope: None,
            item: Identifier::new(format!("sample-{index}")).unwrap(),
            source: None,
            source_time: None,
            payload: Sample { key }.encode(),
        })
        .collect::<Vec<_>>();
    let definition = IndexDefinition {
        name: Identifier::new("by-key").unwrap(),
        version: 1,
        source: ColumnSource { kind: ColumnKind::Records, schema },
        columns: vec!["payload.key".into()],
    };
    let work = ReadLimits { rows: nz(32), bytes: nz(1024 * 1024) };
    let cancel = Cancellation::default();
    let mutation = MutationHeader {
        mutation: MutationId {
            window: Identifier::new("definition-window").unwrap(),
            key: Identifier::new("create-index").unwrap(),
        },
        fingerprint: DefinitionInput::Index {
            value: definition.clone(),
            expected: None,
            rebuild: false,
            work,
        }
        .fingerprint()
        .unwrap(),
        expires_at: std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
    };
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.create_session().await.unwrap();
        backend.append_schemas(StorageBatch::new(vec![descriptor.clone()]).into()).await.unwrap();
        backend.append_records(StorageBatch::new(records.clone())).await.unwrap();
        let before = backend.get_work().await.unwrap();
        assert_eq!(
            backend
                .set_index_definition(&definition, None, false, work, &cancel, Some(&mutation))
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Index))
                .unwrap(),
            backend
                .set_index_definition(&definition, Some(1), false, work, &cancel, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Index))
                .unwrap()
        );
        let after = backend.get_work().await.unwrap();
        if backend.capabilities().native_index {
            assert_eq!(after.source_rows - before.source_rows, records.len() as u64);
            assert_eq!(after.decoded_chunks - before.decoded_chunks, (records.len() / settings.chunk_rows.get()) as u64);
            assert_eq!(after.index_prepares - before.index_prepares, 1);
        }
    }
    let cut = StorageCursor { history: history.clone(), sequence: 9 };
    let saved = native.create_snapshot(&cut).await.unwrap();
    let saved_fallback = fallback.create_snapshot(&cut).await.unwrap();
    let read = IndexRead {
        revision: None,
        name: definition.name.clone(),
        lower: None,
        upper: None,
        prefix: vec![],
        descending: false,
        after: None,
        limits: ReadLimits { rows: nz(2), bytes: nz(4096) },
        work,
    };
    let first = list_index_entries(&native, &saved, &read, &cancel).await.unwrap();
    assert_eq!(first.entries.iter().map(|entry| entry.position).collect::<Vec<_>>(), vec![3, 4]);

    // Append A New Leading Key And Finish Both Saved Seek Journeys.
    let mut late = records[0].clone();
    late.position.sequence = 10;
    late.payload = Sample { key: 0 }.encode();
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.append_records(StorageBatch::new(vec![late.clone()])).await.unwrap();
    }
    for (backend, snapshot) in [(&native as &dyn StorageBackend, &saved), (&fallback as &dyn StorageBackend, &saved_fallback)] {
        let mut page_read = read.clone();
        let mut positions = vec![];
        loop {
            let page = list_index_entries(backend, snapshot, &page_read, &cancel).await.unwrap();
            positions.extend(page.entries.iter().map(|entry| entry.position));
            if page.complete {
                break;
            }
            page_read.after = page.next;
        }
        assert_eq!(positions, vec![3, 4, 6, 2, 7, 8, 5, 9]);
        let mut descending = read.clone();
        descending.descending = true;
        descending.limits = work;
        let page = list_index_entries(backend, snapshot, &descending, &cancel).await.unwrap();
        assert_eq!(page.entries.iter().map(|entry| entry.position).collect::<Vec<_>>(), vec![9, 5, 8, 7, 2, 6, 4, 3]);
        descending.limits = read.limits;
        let mut positions = Vec::new();
        loop {
            let page = list_index_entries(backend, snapshot, &descending, &cancel).await.unwrap();
            positions.extend(page.entries.iter().map(|entry| entry.position));
            if page.complete {
                break;
            }
            descending.after = page.next;
        }
        assert_eq!(positions, vec![9, 5, 8, 7, 2, 6, 4, 3]);
        let mut prefix = read.clone();
        prefix.prefix = encode_key(&[ScalarValue::Unsigned(1)]).unwrap();
        prefix.limits = work;
        let page = list_index_entries(backend, snapshot, &prefix, &cancel).await.unwrap();
        assert_eq!(page.entries.iter().map(|entry| entry.position).collect::<Vec<_>>(), vec![3, 4, 6]);
        let mut empty = read.clone();
        empty.lower = Some(encode_key(&[ScalarValue::Unsigned(100)]).unwrap());
        assert!(list_index_entries(backend, snapshot, &empty, &cancel).await.unwrap().entries.is_empty());
        let mut missing = read.clone();
        missing.name = Identifier::new("missing").unwrap();
        assert!(matches!(list_index_entries(backend, snapshot, &missing, &cancel).await, Err(Error::IndexNotFound { .. })));
    }

    // Invalidate Continuations Explicitly And Rebuild Both Index Owners.
    let mut resumed = read.clone();
    resumed.after = first.next.clone();
    resumed.descending = true;
    assert!(list_index_entries(&native, &saved, &resumed, &cancel).await.is_err());
    let mut rebuilt = definition.clone();
    rebuilt.version += 1;
    for backend in [&native as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        assert!(
            backend
                .set_index_definition(&rebuilt, Some(1), false, work, &cancel, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Index))
                .is_err()
        );
        backend
            .set_index_definition(&rebuilt, Some(1), true, work, &cancel, None)
            .await
            .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Index))
            .unwrap();
        assert!(matches!(
            backend
                .set_index_definition(&rebuilt, Some(1), true, work, &cancel, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Index))
                .unwrap_err()
                .backend_failure(),
            Some(BackendFailure::Conflict)
        ));
        assert!(
            backend
                .set_index_definition(&rebuilt, None, true, work, &cancel, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Index))
                .is_err()
        );
        assert_eq!(backend.get_index_definition(&rebuilt.name).await.unwrap().0, rebuilt);
    }
    resumed.descending = false;
    assert!(list_index_entries(&native, &saved, &resumed, &cancel).await.is_err());
    native.delete_session().await.unwrap();
    drop(native);

    // Reopen The Durable Catalog And Acquire A Fresh Saved-Cut Token.
    let native = SqliteBackend::new(root.path().join("index.sqlite"), history, settings).unwrap();
    native.create_session().await.unwrap();
    assert_eq!(native.get_index_definition(&definition.name).await.unwrap().0, rebuilt);
    // The Durable Receipt Keeps The Original Result After A Later Rebuild And Restart.
    assert_eq!(
        native.set_index_definition(&definition, None, false, work, &cancel, Some(&mutation)).await.unwrap(),
        DefinitionOutcome::Applied { revision: 1 }
    );
    assert_eq!(native.get_index_definition(&definition.name).await.unwrap().0, rebuilt);
    let mut changed_identity = mutation.clone();
    changed_identity.fingerprint = DefinitionInput::Index {
        value: rebuilt.clone(),
        expected: None,
        rebuild: false,
        work,
    }
    .fingerprint()
    .unwrap();
    assert_eq!(
        native.set_index_definition(&rebuilt, None, false, work, &cancel, Some(&changed_identity)).await.unwrap_err().backend_failure(),
        Some(BackendFailure::Conflict)
    );

    assert!(list_index_entries(&native, &saved, &read, &cancel).await.is_err());
    let fresh = native.create_snapshot(&cut).await.unwrap();
    resumed.after = first.next;
    assert!(list_index_entries(&native, &fresh, &resumed, &cancel).await.is_err());
    let mut all = read;
    all.limits = work;
    assert_eq!(list_index_entries(&native, &fresh, &all, &cancel).await.unwrap().entries.len(), records.len());
    native.delete_snapshot(&fresh).await.unwrap();
    native.delete_session().await.unwrap();
    fallback.delete_snapshot(&saved_fallback).await.unwrap();
    fallback.delete_session().await.unwrap();
}
