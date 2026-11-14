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
    num::NonZeroUsize,
    sync::{Arc, atomic::Ordering},
    time::{Duration, UNIX_EPOCH},
};

// Third Party Imports

// Local Imports
use crate::manager::{backend, fixture};
use harmos_domain::model::storage::{catalog::object_schema, object_key};
use harmos_domain::{Identified, Message, schema::Described};
use harmos_query::columnar::ArrowScan;
use harmos_query::columnar::Cancellation;
use harmos_query::columnar::ColumnKind;
use harmos_query::columnar::ColumnSource;
use harmos_query::index::IndexDefinition;
use harmos_query::index::IndexRead;
use harmos_storage::query::ObjectSelection;
use harmos_storage::query::ObjectVersions;
use harmos_storage::query::SchemaSelection;
use harmos_storage::query::index::list_index_entries;
use harmos_storage::query::projection::ProjectionDefinition;
use harmos_storage::query::projection::ProjectionRead;
use harmos_storage::query::projection::ProjectionTransform;
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default, PartialEq)]
struct Counter {
    id: u64,
    value: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Identified for Counter {
    const ID: &'static str = "test.counter";
}
impl StoredObject for Counter {
    type Key = u64;
    const RETENTION: ObjectRetention = ObjectRetention::Latest;
    fn key(&self) -> &u64 {
        &self.id
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn catalog() -> ObjectCatalog {
    ObjectCatalog::new().entry(ObjectDefinition::of::<Counter>())
}

/* === Tests === */

#[tokio::test]
async fn conditional_objects_keep_conflicts_indexes_and_receipts_atomic_across_restart() {
    // Start Both Managers On A Fixed Host Clock.
    let history = backend::history();
    let policy = backend::storage_settings();
    let (clock, _driver) = harmos_storage::execution::Execution::virtual_at(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    let primary = Arc::new(backend::Backend::with_execution(catalog(), history.clone(), clock.clone()));
    let deterministic = Arc::new(backend::Backend::with_execution(catalog(), history.clone(), clock.clone()));
    let work = ReadLimits { rows: nz(64), bytes: nz(1024 * 1024) };
    let cancel = Cancellation::default();
    let mut retained = None;
    let definition = IndexDefinition {
        name: Identifier::new("counter-value").unwrap(),
        version: 1,
        source: ColumnSource {
            kind: ColumnKind::Objects,
            schema: object_schema::<Counter>().unwrap(),
        },
        columns: vec!["payload.value".into()],
    };
    for primary in [primary.clone() as Arc<dyn StorageBackend>, deterministic.clone() as Arc<dyn StorageBackend>] {
        let storage = StorageManager::with_catalog(
            catalog(),
            StorageBackends {
                primary: primary.clone(),
                state: Arc::new(backend::RecoveryBackend::new(history.clone())),
            },
            policy.clone(),
        )
        .unwrap();
        storage.adopt_clock(&clock).unwrap();
        fixture::start(&storage).await;
        let window = storage.mutation_window().unwrap().id;
        let create = MutationId {
            window: window.clone(),
            key: Identifier::new("create").unwrap(),
        };
        let value = Counter { id: 1, value: 1 };
        let outcome = storage.compare_exchange_once(&create, None, &value).await.unwrap();
        let CompareExchange::Applied(initial) = outcome.clone() else {
            panic!("creation must apply");
        };
        assert_eq!(storage.compare_exchange_once(&create, None, &value).await.unwrap(), outcome);
        assert!(storage.compare_exchange_once(&create, None, &Counter { id: 1, value: 99 }).await.is_err());
        let saved = primary.create_snapshot(&initial.position).await.unwrap();
        storage.set_index_definition(&definition, None, false, work, &cancel).await.unwrap();

        // Scope Expected Tokens To Their Object And Refuse Existing Creates.
        assert!(matches!(
            storage.compare_exchange(Some(initial.version.clone()), &Counter { id: 2, value: 2 }).await.unwrap(),
            CompareExchange::Conflict { current: None }
        ));
        assert!(storage.get_object::<Counter>(&2, ObjectVersion::Latest).await.unwrap().is_none());
        let existing = MutationId {
            window: window.clone(),
            key: Identifier::new("existing").unwrap(),
        };
        let existing_outcome = storage.compare_exchange_once(&existing, None, &Counter { id: 1, value: 99 }).await.unwrap();
        assert_eq!(existing_outcome, CompareExchange::Conflict { current: Some(initial.version.clone()) });

        // Let Two Callers Replace The Same Committed Version.
        let left_value = Counter { id: 1, value: 10 };
        let right_value = Counter { id: 1, value: 20 };
        let (left, right) = tokio::join!(
            storage.compare_exchange(Some(initial.version.clone()), &left_value),
            storage.compare_exchange(Some(initial.version.clone()), &right_value)
        );
        let results = [(left.unwrap(), left_value.value), (right.unwrap(), right_value.value)];
        assert_eq!(results.iter().filter(|(outcome, _)| matches!(outcome, CompareExchange::Applied(_))).count(), 1);
        let (winner, winner_value) = results.iter().find(|(outcome, _)| matches!(outcome, CompareExchange::Applied(_))).unwrap();
        let CompareExchange::Applied(winner) = winner else { unreachable!() };
        let current = storage.get_object::<Counter>(&1, ObjectVersion::Latest).await.unwrap().unwrap();
        assert_eq!(current.value.value, *winner_value);
        assert_eq!(current.version, winner.version);
        for (outcome, _) in &results {
            if let CompareExchange::Conflict { current } = outcome {
                assert_eq!(current.as_ref(), Some(&winner.version));
            }
        }
        assert_eq!(storage.compare_exchange_once(&create, None, &value).await.unwrap(), CompareExchange::Applied(initial.clone()));
        assert_eq!(storage.compare_exchange_once(&existing, None, &Counter { id: 1, value: 99 }).await.unwrap(), existing_outcome);

        // Observe One Latest Index Entry And No Schema Row From Either Conflict.
        let now = primary.create_snapshot(&storage.get_saved_position()).await.unwrap();
        let read = IndexRead {
            revision: None,
            name: definition.name.clone(),
            lower: None,
            upper: None,
            prefix: vec![],
            descending: false,
            after: None,
            limits: work,
            work,
        };
        assert_eq!(
            list_index_entries(primary.as_ref(), &now, &read, &cancel)
                .await
                .unwrap()
                .entries
                .iter()
                .map(|entry| entry.position)
                .collect::<Vec<_>>(),
            vec![winner.position.sequence]
        );
        assert_eq!(
            list_index_entries(primary.as_ref(), &saved, &read, &cancel)
                .await
                .unwrap()
                .entries
                .iter()
                .map(|entry| entry.position)
                .collect::<Vec<_>>(),
            vec![initial.position.sequence]
        );
        let schemas = primary
            .list_schemas(
                &now,
                &SchemaSelection {
                    schema: Some(definition.source.schema.clone()),
                },
                ScanPage {
                    after: 0,
                    rows: work.rows,
                    bytes: work.bytes,
                },
            )
            .await
            .unwrap();
        assert_eq!(schemas.len(), 2);
        let old = primary
            .list_object_revisions(
                &saved,
                &ObjectSelection {
                    schema: Some(definition.source.schema.clone()),
                    key: None,
                    versions: ObjectVersions::Latest,
                },
                ScanPage {
                    after: 0,
                    rows: work.rows,
                    bytes: work.bytes,
                },
            )
            .await
            .unwrap();
        assert_eq!(Counter::decode(&old[0].value.payload).unwrap(), value);
        primary.delete_snapshot(&now).await.unwrap();
        primary.delete_snapshot(&saved).await.unwrap();

        // Use The Public Snapshot Resources Without Escaping To A Backend.
        let held = storage.snapshot(&storage.get_saved_position()).await.unwrap();
        let scan = IndexRead {
            revision: None,
            name: definition.name.clone(),
            lower: None,
            upper: None,
            prefix: vec![],
            descending: false,
            after: None,
            limits: work,
            work,
        };
        assert_eq!(held.list_index_entries(&scan, &cancel).await.unwrap().entries.len(), 1);
        let mut scan = ArrowScan {
            source: definition.source.clone(),
            columns: vec!["payload.value".into()],
            predicate: None,
            time: None,
            start: 0,
            after: None,
            limits: work,
            work,
        };
        let mut values = Vec::new();
        loop {
            let page = held.list_column_rows(&scan, &cancel).await.unwrap();
            values.extend_from_slice(page.batch.column(0).as_any().downcast_ref::<arrow_array::UInt64Array>().unwrap().values());
            if page.complete {
                break;
            }
            scan.after = page.next;
        }
        assert_eq!(values, vec![*winner_value]);
        let projection = ProjectionDefinition {
            name: Identifier::new("counter-projection").unwrap(),
            version: 1,
            source: definition.source.clone(),
            transform: ProjectionTransform::Select(vec!["payload.value".into()]),
        };
        let revision = storage.set_projection_definition(&projection, None, false).await.unwrap();
        held.advance_projection(&projection.name, None, work, &cancel).await.unwrap();
        assert_eq!(storage.get_projection_status(&projection.name, revision, Some(held.at())).await.unwrap().unwrap().rows, 1);
        let page = storage
            .list_projection_rows(&ProjectionRead {
                name: projection.name.clone(),
                revision,
                frontier: held.at().clone(),
                after: 0,
                limits: work,
                work,
            })
            .await
            .unwrap();
        assert_eq!(page.batch.num_rows(), 1);
        storage.delete_projection_history(&projection.name, revision, held.at()).await.unwrap();
        drop(held);

        // A Dropped Caller Cannot Retract Work Already Admitted To The Worker.
        if Arc::ptr_eq(&primary, &(deterministic.clone() as Arc<dyn StorageBackend>)) {
            deterministic.control.block_write.store(true, Ordering::SeqCst);
            let mutation = MutationId {
                window,
                key: Identifier::new("cancelled").unwrap(),
            };
            let task = {
                let storage = storage.clone();
                let mutation = mutation.clone();
                let expected = winner.version.clone();
                tokio::spawn(async move { storage.compare_exchange_once(&mutation, Some(expected), &Counter { id: 1, value: 30 }).await })
            };
            deterministic.control.write_entered.notified().await;
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            deterministic.control.block_write.store(false, Ordering::SeqCst);
            deterministic.control.write_release.notify_one();
            storage.flush().await.unwrap();
            assert!(matches!(
                storage.compare_exchange_once(&mutation, Some(winner.version.clone()), &Counter { id: 1, value: 30 }).await.unwrap(),
                CompareExchange::Applied(_)
            ));
            assert_eq!(storage.get_object::<Counter>(&1, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 30);
        } else {
            retained = Some((create, existing, initial, existing_outcome, winner.clone()));
        }
        storage.close().await.unwrap();
    }

    // Abort An Index Update Inside The Real Object Transaction, Then Retry It.
    let (create, existing, initial, existing_outcome, winner) = retained.unwrap();
    primary.create_session().await.unwrap();
    let before = primary.get_saved_position().await.unwrap();
    while !primary.delete_expired_object_revisions(&before, nz(64), &catalog()).await.unwrap() {}
    let pruned = primary.create_snapshot(&before).await.unwrap();
    let point = ObjectPoint {
        schema: definition.source.schema.id.clone(),
        item: object_key(&1_u64, work.bytes).unwrap(),
        version: ObjectVersion::Exact(initial.version.clone()),
    };
    assert!(matches!(primary.get_object(&pruned, &point).await, Err(Error::Gap { .. })));
    primary.delete_snapshot(&pruned).await.unwrap();
    let value = Counter { id: 1, value: 333 };
    let graph =
        harmos_domain::schema::graph::Schema::from_declaration(Counter::DEFINITION, &harmos_domain::config::SchemaSettings::default())
            .unwrap();
    let position = StorageCursor {
        history: history.clone(),
        sequence: before.sequence + 2,
    };
    let exchange = ObjectExchange {
        expected: Some(winner.version),
        schema: StorageSchema::new(
            StorageCursor {
                history: history.clone(),
                sequence: before.sequence + 1,
            },
            definition.source.schema.clone(),
            graph,
        ),
        value: ObjectEntry {
            version: ObjectVersionToken::new(position.clone()),
            position: position.clone(),
            value: ObjectValue {
                schema: definition.source.schema.clone(),
                item: object_key(&value.id, work.bytes).unwrap(),
                payload: value.encode(),
            },
        },
    };
    primary.control.permanent.store(true, Ordering::SeqCst);
    assert!(primary.set_object(&exchange).await.is_err());
    assert_eq!(primary.get_saved_position().await.unwrap(), before);
    primary.control.permanent.store(false, Ordering::SeqCst);
    let accepted = primary.set_object(&exchange).await.unwrap();
    assert!(matches!(accepted, CompareExchange::Applied(_)));
    assert_eq!(primary.set_object(&exchange).await.unwrap(), accepted);
    let mut forged = exchange.clone();
    forged.value.value.payload = Counter { id: 1, value: 444 }.encode();
    assert!(primary.set_object(&forged).await.is_err());
    primary.delete_session().await.unwrap();

    // Restart, Replay Original Success And Conflict, Then Retire Acknowledged Outcomes.
    let storage = StorageManager::with_catalog(
        catalog(),
        StorageBackends {
            primary: primary.clone(),
            state: Arc::new(backend::RecoveryBackend::new(history)),
        },
        policy,
    )
    .unwrap();
    storage.adopt_clock(&clock).unwrap();
    fixture::start(&storage).await;
    assert_eq!(
        storage.compare_exchange_once(&create, None, &Counter { id: 1, value: 1 }).await.unwrap(),
        CompareExchange::Applied(initial.clone())
    );
    assert_eq!(storage.compare_exchange_once(&existing, None, &Counter { id: 1, value: 99 }).await.unwrap(), existing_outcome);
    assert_eq!(storage.get_object::<Counter>(&1, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 333);
    storage.delete_object_receipts(&initial.position).await.unwrap();
    assert!(matches!(storage.compare_exchange_once(&create, None, &Counter { id: 1, value: 1 }).await, Err(Error::Gap { .. })));
    assert_eq!(storage.get_object::<Counter>(&1, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 333);
    storage.close().await.unwrap();
}
