/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Prove Deletion Isolation, Replay and Retained Reclamation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, UNIX_EPOCH};

// Third Party Imports
use harmos_query::columnar::{Cancellation, ColumnKind, ColumnSource};

// Local Imports
use super::{Counter, catalog, compound_value};
use crate::manager::{backend, events, fixture};
use harmos_storage::conformance::{self, ScopeFixture};
use harmos_storage::execution::Execution;
use harmos_storage::query::projection::{self, ProjectionDefinition, ProjectionRead, ProjectionTransform};
use harmos_storage::query::{FileSelection, RecordSelection, StreamSelection};
use harmos_storage::{
    CompoundOutcome, FileMetadata, Identifier, MutationId, MutationItem, ObjectPoint, ObjectVersion, ObjectVersionToken, ReadLimits,
    ScanPage, StorageBackend, StorageBackends, StorageManager, StreamInput,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn start() -> (StorageManager, Arc<backend::Backend>) {
    let history = backend::history();
    let (clock, _driver) = Execution::virtual_at(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    let primary = Arc::new(backend::Backend::with_execution(catalog(), history.clone(), clock.clone()));
    let storage = StorageManager::with_catalog(
        catalog(),
        StorageBackends {
            primary: primary.clone(),
            state: Arc::new(backend::RecoveryBackend::new(history)),
        },
        backend::storage_settings(),
    )
    .unwrap();
    storage.adopt_clock(&clock).unwrap();
    fixture::start(&storage).await;
    (storage, primary)
}

fn mutation(storage: &StorageManager, key: &str) -> MutationId {
    MutationId {
        window: storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}

fn page() -> ScanPage {
    ScanPage {
        after: 0,
        rows: NonZeroUsize::new(64).unwrap(),
        bytes: NonZeroUsize::new(64 * 1024).unwrap(),
    }
}

/* === Tests === */

#[tokio::test]
async fn object_delete_hides_at_new_cuts_only() {
    // Seed Two Independently Versioned Objects.
    let (storage, primary) = start().await;
    let first = storage.write_object(&Counter { id: 1, value: 10 }).await.unwrap();
    let second = storage.write_object(&Counter { id: 2, value: 20 }).await.unwrap();
    storage.flush_until(&second.position).await.unwrap();
    let before = primary.create_snapshot(&second.position).await.unwrap();
    let value = compound_value(1, 10);
    let point = ObjectPoint {
        schema: value.schema.id.clone(),
        item: value.item.clone(),
        version: ObjectVersion::Latest,
    };

    // A Wrong Revision Changes Nothing and Has a Replayable Outcome.
    let refused = mutation(&storage, "wrong-delete");
    let items = vec![MutationItem::Delete {
        schema: point.schema.clone(),
        item: point.item.clone(),
        expected: Some(second.version),
    }];
    let conflict = storage.mutate_once(&refused, items.clone()).await.unwrap();
    assert_eq!(
        conflict.outcome,
        CompoundOutcome::Conflict {
            index: 0,
            current: Some(first.version.clone())
        }
    );
    assert_eq!(storage.mutate_once(&refused, items).await.unwrap(), conflict);

    // The Correct Revision Hides Latest and Exact Reads Only at New Cuts.
    let deleted = storage
        .mutate_once(
            &mutation(&storage, "delete"),
            vec![MutationItem::Delete {
                schema: point.schema.clone(),
                item: point.item.clone(),
                expected: Some(first.version.clone()),
            }],
        )
        .await
        .unwrap();
    let after = primary.create_snapshot(&deleted.position).await.unwrap();
    assert!(primary.get_object(&after, &point).await.unwrap().is_none());
    let exact = ObjectPoint {
        version: ObjectVersion::Exact(first.version),
        ..point.clone()
    };
    assert!(primary.get_object(&after, &exact).await.unwrap().is_none());
    assert_eq!(primary.get_object(&before, &point).await.unwrap().unwrap().value, value);
    assert!(primary.get_object(&before, &exact).await.unwrap().is_some());
    primary.delete_snapshot(&after).await.unwrap();
    primary.delete_snapshot(&before).await.unwrap();
    storage.close().await.unwrap();
}

#[tokio::test]
async fn scoped_delete_reclaims_after_cut_release() {
    // Seed Objects, Records and Samples in Two Opaque Scopes.
    let (storage, primary) = start().await;
    let scope = Identifier::new("selected").unwrap();
    let other = Identifier::new("selected-other").unwrap();
    let mut first = compound_value(1, 10);
    first.scope = Some(scope.clone());
    let mut second = compound_value(2, 20);
    second.scope = Some(other.clone());
    let mut record = fixture::record(&first.schema, &Identifier::new("first").unwrap(), &first.payload);
    record.scope = Some(scope.clone());
    let mut kept = fixture::record(&first.schema, &Identifier::new("kept").unwrap(), &second.payload);
    kept.scope = Some(other.clone());
    storage
        .mutate_once(
            &mutation(&storage, "seed"),
            vec![
                MutationItem::Create(first.clone()),
                MutationItem::Create(second.clone()),
                MutationItem::Append(record.clone()),
                MutationItem::Append(kept.clone()),
            ],
        )
        .await
        .unwrap();
    storage
        .append_samples(vec![
            StreamInput {
                record,
                stream: Identifier::new("samples").unwrap(),
                sequence: 1,
            },
            StreamInput {
                record: kept,
                stream: Identifier::new("samples").unwrap(),
                sequence: 2,
            },
        ])
        .await
        .unwrap();

    // Finish a Scoped File and Pin the Complete Original Cut.
    let metadata = FileMetadata {
        id: Identifier::new("scoped-file").unwrap(),
        scope: Some(scope.clone()),
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    };
    let mut writer = storage.produce_file(metadata).await.unwrap();
    writer.write_all(b"retained".to_vec()).await.unwrap();
    let file = writer.finish().await.unwrap();
    storage.flush().await.unwrap();
    let before = primary.create_snapshot(&storage.get_saved_position()).await.unwrap();
    let records = RecordSelection {
        schema: first.schema.clone(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    };
    let samples = StreamSelection {
        schema: first.schema.clone(),
        scope: None,
        item: None,
        stream: None,
        source_start: None,
        source_end: None,
        after: 0,
    };
    let files = FileSelection { scope: None, item: None };
    let point = ObjectPoint {
        schema: first.schema.id.clone(),
        item: first.item.clone(),
        version: ObjectVersion::Latest,
    };

    // Retain a Derived Projection at the Same Original Cut.
    let definition = ProjectionDefinition {
        name: Identifier::new("scoped-projection").unwrap(),
        version: 1,
        source: ColumnSource {
            kind: ColumnKind::Records,
            schema: first.schema.clone(),
        },
        transform: ProjectionTransform::Select(vec!["_storage.scope".into(), "payload.value".into()]),
    };
    storage.set_projection_definition(&definition, None, false).await.unwrap();
    let work = ReadLimits { rows: page().rows, bytes: page().bytes };
    let cancel = Cancellation::default();
    let revision = projection::advance_projection(primary.as_ref(), &before, &definition.name, None, work, &cancel).await.unwrap();
    let prior_projection = ProjectionRead {
        name: definition.name.clone(),
        revision,
        frontier: before.at.clone(),
        after: 0,
        limits: work,
        work,
    };
    assert_eq!(storage.list_projection_rows(&prior_projection).await.unwrap().batch.num_rows(), 2);

    // Delete Exactly the Selected Scope and Replay Its Original Cut.
    let identity = mutation(&storage, "delete-scope");
    let items = vec![MutationItem::DeleteScope(scope)];
    let deleted = storage.mutate_once(&identity, items.clone()).await.unwrap();
    assert_eq!(storage.mutate_once(&identity, items).await.unwrap(), deleted);
    let after = primary.create_snapshot(&deleted.position).await.unwrap();
    assert!(primary.get_object(&after, &point).await.unwrap().is_none());
    let kept = ObjectPoint { item: second.item, ..point.clone() };
    assert!(primary.get_object(&after, &kept).await.unwrap().is_some());
    let remaining = primary.list_records(&after, &records, page()).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].scope.as_ref(), Some(&other));
    let remaining = primary.list_samples(&after, &samples, page()).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].record.scope.as_ref(), Some(&other));
    assert!(primary.list_files(&after, &files, page()).await.unwrap().is_empty());

    // New Projection Frontiers Exclude the Deleted Scope.
    projection::advance_projection(primary.as_ref(), &after, &definition.name, Some(revision), work, &cancel).await.unwrap();
    let current_projection = ProjectionRead {
        frontier: after.at.clone(),
        ..prior_projection.clone()
    };
    assert_eq!(storage.list_projection_rows(&current_projection).await.unwrap().batch.num_rows(), 1);

    // Retained Cuts Keep Their Objects and Bytes Until Release.
    assert!(primary.get_object(&before, &point).await.unwrap().is_some());
    assert_eq!(primary.list_records(&before, &records, page()).await.unwrap().len(), 2);
    assert_eq!(primary.list_samples(&before, &samples, page()).await.unwrap().len(), 2);
    assert_eq!(primary.get_file_bytes(&before, &file, 0, NonZeroUsize::new(64).unwrap()).await.unwrap(), b"retained");
    assert!(primary.retained_file_bytes() >= 8);
    let shared = primary.create_snapshot(&before.at).await.unwrap();
    assert_eq!(primary.get_file_bytes(&shared, &file, 0, page().bytes).await.unwrap(), b"retained");
    primary.delete_snapshot(&before).await.unwrap();
    primary.delete_snapshot(&after).await.unwrap();
    assert!(primary.retained_file_bytes() >= 8);
    primary.delete_snapshot(&shared).await.unwrap();
    assert!(primary.create_snapshot(&shared.at).await.is_err());
    primary.snapshots_released().await;
    assert_eq!(primary.retained_file_bytes(), 0);
    assert!(storage.list_projection_rows(&prior_projection).await.is_err());
    assert_eq!(storage.list_projection_rows(&current_projection).await.unwrap().batch.num_rows(), 1);
    storage.close().await.unwrap();
}

#[tokio::test]
async fn scoped_delete_replays_receipt() {
    // Commit a Deletion Then Write New Data Into the Same Scope.
    let (storage, primary) = start().await;
    let scope = Identifier::new("reopened-scope").unwrap();
    let identity = mutation(&storage, "delete-scope");
    let items = vec![MutationItem::DeleteScope(scope.clone())];
    let deleted = storage.mutate_once(&identity, items.clone()).await.unwrap();
    let mut value = compound_value(1, 20);
    value.scope = Some(scope);
    storage.mutate_once(&mutation(&storage, "new-data"), vec![MutationItem::Create(value.clone())]).await.unwrap();

    // Replay Cannot Delete Data Written After Its Original Frontier.
    assert_eq!(storage.mutate_once(&identity, items).await.unwrap(), deleted);
    let snapshot = primary.create_snapshot(&storage.get_saved_position()).await.unwrap();
    let point = ObjectPoint {
        schema: value.schema.id,
        item: value.item,
        version: ObjectVersion::Latest,
    };
    assert!(primary.get_object(&snapshot, &point).await.unwrap().is_some());
    primary.delete_snapshot(&snapshot).await.unwrap();
    storage.close().await.unwrap();
}

#[tokio::test]
async fn active_production_refuses_scope_deletion_before_admission() {
    // Keep a Producer Open in the Selected Scope.
    let (storage, primary) = start().await;
    let scope = Identifier::new("active-scope").unwrap();
    let metadata = FileMetadata {
        id: Identifier::new("active-file").unwrap(),
        scope: Some(scope.clone()),
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    };
    let mut writer = storage.produce_file(metadata).await.unwrap();
    let before = storage.status().accepted;
    let identity = mutation(&storage, "delete-active-scope");
    let items = vec![MutationItem::DeleteScope(scope)];
    assert!(storage.mutate_once(&identity, items.clone()).await.is_err());
    assert_eq!(storage.status().accepted, before);

    // Finishing the Producer Makes the Same Unadmitted Identity Usable.
    writer.finish().await.unwrap();
    let result = storage.mutate_once(&identity, items).await.unwrap();
    assert!(matches!(result.outcome, CompoundOutcome::Applied { .. }));
    primary.snapshots_released().await;
    storage.close().await.unwrap();
}

#[tokio::test]
async fn scoped_delete_does_not_resurrect_prior_ownership() {
    let (storage, primary) = start().await;
    let earlier = Identifier::new("earlier-owner").unwrap();
    let current = Identifier::new("current-owner").unwrap();
    let mut value = compound_value(1, 10);
    value.scope = Some(earlier.clone());
    let created = storage.mutate_once(&mutation(&storage, "create"), vec![MutationItem::Create(value.clone())]).await.unwrap();
    let mut updated = compound_value(1, 20);
    updated.scope = Some(current.clone());
    storage
        .mutate_once(
            &mutation(&storage, "move-owner"),
            vec![MutationItem::Set {
                expected: ObjectVersionToken::new(created.position),
                value: updated.clone(),
            }],
        )
        .await
        .unwrap();

    // Deleting Historical Ownership Keeps the Current Revision.
    storage.mutate_once(&mutation(&storage, "delete-earlier"), vec![MutationItem::DeleteScope(earlier)]).await.unwrap();
    let point = ObjectPoint {
        schema: value.schema.id,
        item: value.item,
        version: ObjectVersion::Latest,
    };
    let snapshot = primary.create_snapshot(&storage.get_saved_position()).await.unwrap();
    assert_eq!(primary.get_object(&snapshot, &point).await.unwrap().unwrap().value, updated);
    primary.delete_snapshot(&snapshot).await.unwrap();

    // Deleting Current Ownership Cannot Reveal an Older Revision.
    storage.mutate_once(&mutation(&storage, "delete-current"), vec![MutationItem::DeleteScope(current)]).await.unwrap();
    let snapshot = primary.create_snapshot(&storage.get_saved_position()).await.unwrap();
    assert!(primary.get_object(&snapshot, &point).await.unwrap().is_none());
    primary.delete_snapshot(&snapshot).await.unwrap();
    storage.close().await.unwrap();
}

#[tokio::test]
async fn accepted_scope_delete_fences_creation_through_cancellation() {
    let (storage, primary) = start().await;
    let scope = Identifier::new("pending-scope").unwrap();
    let identity = mutation(&storage, "pending-delete");
    let items = vec![MutationItem::DeleteScope(scope.clone())];
    primary.control.block_write.store(true, Ordering::SeqCst);
    let mut original = Box::pin(storage.mutate_once(&identity, items.clone()));
    assert!(events::until(original.as_mut(), primary.control.write_entered.notified()).await.is_ok());
    drop(original);

    // Accepted Work Owns the Fence After Its Caller Is Gone.
    let metadata = FileMetadata {
        id: Identifier::new("later-file").unwrap(),
        scope: Some(scope),
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    };
    assert!(storage.produce_file(metadata.clone()).await.is_err());
    assert!(storage.create_file_once(&mutation(&storage, "later-create"), &metadata).await.is_err());

    // Receipt Retry Uses the Existing Accepted Work Without a Second Lock.
    primary.control.block_write.store(false, Ordering::SeqCst);
    primary.control.write_release.notify_one();
    let deleted = storage.mutate_once(&identity, items).await.unwrap();
    assert!(matches!(deleted.outcome, CompoundOutcome::Applied { .. }));
    let mut writer = storage.produce_file(metadata).await.unwrap();
    writer.finish().await.unwrap();
    storage.close().await.unwrap();
}

#[tokio::test]
async fn memory_backend_passes_scope_contract() {
    let (storage, _primary) = start().await;
    let scope = Identifier::new("conformance-scope").unwrap();
    let mut object = compound_value(1, 10);
    object.scope = Some(scope.clone());
    let mut record = fixture::record(&object.schema, &Identifier::new("scoped-record").unwrap(), &object.payload);
    record.scope = Some(scope.clone());
    let sample = StreamInput {
        record: record.clone(),
        stream: Identifier::new("scoped-stream").unwrap(),
        sequence: 1,
    };
    let file = FileMetadata {
        id: Identifier::new("scoped-file").unwrap(),
        scope: Some(scope.clone()),
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    };
    let fixture = ScopeFixture {
        scope,
        object,
        record,
        sample,
        file,
        bytes: b"scoped-file-bytes".to_vec(),
    };
    conformance::scoped_deletion(&storage, fixture).await.unwrap();
    storage.close().await.unwrap();
}
