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
use harmos_query::columnar::Cancellation;
use harmos_query::columnar::ColumnKind;
use harmos_query::columnar::ColumnSource;
use harmos_storage::query::projection::{ProjectionDefinition, ProjectionRead, ProjectionTransform, advance_projection};
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
    let history = backend::history();
    let primary = backend::Backend::new(history.clone());
    let fallback = backend::Backend::new(history.clone());
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
    for backend in [&primary as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.create_session().await.unwrap();
        backend.append_schemas(StorageBatch::new(vec![descriptor.clone()]).into()).await.unwrap();
        backend.append_records(StorageBatch::new(records[..2].to_vec())).await.unwrap();
        assert_eq!(
            backend
                .set_projection_definition(&definition, None, false, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Projection))
                .unwrap(),
            backend
                .set_projection_definition(&definition, Some(1), false, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Projection))
                .unwrap()
        );
    }
    let first_cut = StorageCursor { history: history.clone(), sequence: 3 };
    let first_primary = primary.create_snapshot(&first_cut).await.unwrap();
    let first_fallback = fallback.create_snapshot(&first_cut).await.unwrap();
    let revision = advance_projection(&primary, &first_primary, &definition.name, None, work, &cancel).await.unwrap();
    advance_projection(&fallback, &first_fallback, &definition.name, None, work, &cancel).await.unwrap();
    let old_read = ProjectionRead {
        name: definition.name.clone(),
        revision,
        frontier: first_cut.clone(),
        after: 0,
        limits: work,
        work,
    };

    // Inject A Failure After The First Output Page Has Been Written In The Transaction.
    for backend in [&primary as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        backend.append_records(StorageBatch::new(records[2..].to_vec())).await.unwrap();
    }
    let next_cut = StorageCursor { history: history.clone(), sequence: 6 };
    let next_primary = primary.create_snapshot(&next_cut).await.unwrap();
    let next_fallback = fallback.create_snapshot(&next_cut).await.unwrap();
    primary.control.fail_projection.store(true, Ordering::SeqCst);
    assert!(advance_projection(&primary, &next_primary, &definition.name, None, work, &cancel).await.is_err());
    fallback.control.fail_projection.store(true, Ordering::SeqCst);
    assert!(advance_projection(&fallback, &next_fallback, &definition.name, None, work, &cancel).await.is_err());
    for backend in [&primary as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        assert!(backend.get_projection_status(&definition.name, revision, Some(&next_cut)).await.unwrap().is_none());
        assert_eq!(
            backend.get_projection_status(&definition.name, revision, None).await.unwrap().map(|status| status.frontier),
            Some(first_cut.clone())
        );
    }
    primary.delete_session().await.unwrap();

    // Reopen, Keep The Published Cut, And Replay The Interrupted Candidate Once.
    primary.create_session().await.unwrap();
    let old = primary.list_projection_rows(&old_read).await.unwrap();
    assert_eq!(old.batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap().values().as_ref(), &[0.0, 1.0]);
    primary.control.fail_projection.store(false, Ordering::SeqCst);
    fallback.control.fail_projection.store(false, Ordering::SeqCst);
    let fresh = primary.create_snapshot(&next_cut).await.unwrap();
    assert_eq!(advance_projection(&primary, &fresh, &definition.name, None, work, &cancel).await.unwrap(), revision);
    assert_eq!(advance_projection(&primary, &fresh, &definition.name, None, work, &cancel).await.unwrap(), revision);
    advance_projection(&fallback, &next_fallback, &definition.name, None, work, &cancel).await.unwrap();
    for backend in [&primary as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        assert_eq!(backend.get_projection_status(&definition.name, revision, Some(&next_cut)).await.unwrap().unwrap().rows, records.len());
        let mut read = ProjectionRead {
            frontier: next_cut.clone(),
            limits: ReadLimits { rows: nz(1), bytes: nz(1024) },
            ..old_read.clone()
        };
        let mut values = vec![];
        loop {
            let page = backend.list_projection_rows(&read).await.unwrap();
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
        assert!(matches!(backend.list_projection_rows(&lag).await, Err(Error::NotReady { .. })));
        assert_eq!(backend.list_projection_rows(&old_read).await.unwrap().batch.num_rows(), 2);
    }

    // Rebuild Explicitly, Preserve Prior Revisions, Then Retire Only The Chosen Cut.
    let mut rebuilt = definition.clone();
    rebuilt.version += 1;
    rebuilt.transform = ProjectionTransform::Select(vec!["_storage.position".into(), "payload.value".into()]);
    for backend in [&primary as &dyn StorageBackend, &fallback as &dyn StorageBackend] {
        assert!(
            backend
                .set_projection_definition(&rebuilt, Some(1), false, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Projection))
                .is_err()
        );
        let next_revision = backend
            .set_projection_definition(&rebuilt, Some(1), true, None)
            .await
            .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Projection))
            .unwrap();
        assert_ne!(next_revision, revision);
        assert!(matches!(
            backend
                .set_projection_definition(&rebuilt, Some(1), true, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Projection))
                .unwrap_err()
                .backend_failure(),
            Some(BackendFailure::Conflict)
        ));
        assert!(
            backend
                .set_projection_definition(&rebuilt, None, true, None)
                .await
                .and_then(|outcome| outcome.into_result(harmos_storage::DefinitionKind::Projection))
                .is_err()
        );
        assert_eq!(backend.get_projection_definition(&rebuilt.name, None).await.unwrap().0, rebuilt);
        assert_eq!(backend.list_projection_rows(&old_read).await.unwrap().batch.num_rows(), 2);
        backend.delete_projection_history(&definition.name, revision, &first_cut).await.unwrap();
        assert!(matches!(backend.list_projection_rows(&old_read).await, Err(Error::Gap { .. })));
        let preserved = ProjectionRead {
            frontier: next_cut.clone(),
            ..old_read.clone()
        };
        assert!(backend.list_projection_rows(&preserved).await.is_ok());
    }
    primary.delete_snapshot(&fresh).await.unwrap();
    primary.delete_session().await.unwrap();
    fallback.delete_snapshot(&first_fallback).await.unwrap();
    fallback.delete_snapshot(&next_fallback).await.unwrap();
    fallback.delete_session().await.unwrap();
}
