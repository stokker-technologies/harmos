/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Commit Objects and Records Together

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

// Third Party Imports

// Local Imports
use super::{Counter, catalog, compound_value};
use crate::manager::{backend, fixture};
use harmos_storage::{
    CompoundOutcome, Identifier, MutationId, MutationItem, ObjectPoint, ObjectVersion, StorageBackend, StorageBackends, StorageManager,
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

/* === Tests === */

#[tokio::test]
async fn compound_mutation_commits_atomically() {
    // Start Storage.
    let history = backend::history();
    let (clock, _driver) = harmos_storage::execution::Execution::virtual_at(UNIX_EPOCH + Duration::from_secs(1_700_000_000));
    let primary = Arc::new(backend::Backend::with_execution(catalog(), history.clone(), clock.clone()));
    let recovery = Arc::new(backend::RecoveryBackend::new(history));
    let storage = StorageManager::with_catalog(
        catalog(),
        StorageBackends {
            primary: primary.clone(),
            state: recovery.clone(),
        },
        backend::storage_settings(),
    )
    .unwrap();
    storage.adopt_clock(&clock).unwrap();
    fixture::start(&storage).await;
    let window = storage.mutation_window().unwrap().id;
    // Retain the Original Cut.
    let initial = storage.write_object(&Counter { id: 1, value: 10 }).await.unwrap();
    storage.flush_until(&initial.position).await.unwrap();
    let before = primary.create_snapshot(&initial.position).await.unwrap();
    let mutation = MutationId {
        window,
        key: Identifier::new("compound").unwrap(),
    };
    let mut record = fixture::record(&compound_value(1, 0).schema, &Identifier::new("event").unwrap(), &[7]);
    record.source_time = UNIX_EPOCH.checked_sub(Duration::from_secs(1));

    // Commit Two Objects and One Record Together.
    let reply = storage
        .mutate_once(
            &mutation,
            vec![
                MutationItem::Set {
                    expected: initial.version.clone(),
                    value: compound_value(1, 20),
                },
                MutationItem::Create(compound_value(2, 30)),
                MutationItem::Append(record),
            ],
        )
        .await
        .unwrap();
    let CompoundOutcome::Applied { positions } = reply.outcome else { panic!("compound commit refused") };
    assert_eq!(positions.len(), 3);
    assert_eq!(positions.last(), Some(&reply.position));
    assert_eq!(storage.get_object::<Counter>(&1, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 20);
    assert_eq!(storage.get_object::<Counter>(&2, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 30);
    assert_eq!(primary.records.lock().unwrap().len(), 1);
    assert_eq!(storage.get_saved_position(), reply.position);
    assert!(storage.mutation_durable(&mutation, &reply.position).unwrap());
    assert!(storage.check_receipt_position(&reply.position).unwrap());

    // Retained Cuts Remain Whole and Intermediate Cuts Are Refused.
    let point = ObjectPoint {
        schema: compound_value(1, 0).schema.id,
        item: compound_value(1, 0).item,
        version: ObjectVersion::Latest,
    };
    assert_eq!(primary.get_object(&before, &point).await.unwrap().unwrap().value.payload, compound_value(1, 10).payload);
    assert!(primary.create_snapshot(&positions[0]).await.is_err());
    // Compound Deletion Keeps the Prior Image and Allows a Later Create.
    let after = primary.create_snapshot(&reply.position).await.unwrap();
    let second = compound_value(2, 0);
    let deleted = storage
        .mutate_once(
            &MutationId {
                window: mutation.window.clone(),
                key: Identifier::new("delete").unwrap(),
            },
            vec![
                MutationItem::Delete {
                    schema: second.schema.id.clone(),
                    item: second.item.clone(),
                    expected: Some(harmos_storage::ObjectVersionToken::new(positions[1].clone())),
                },
                MutationItem::Append(fixture::record(&second.schema, &Identifier::new("deleted").unwrap(), &[8])),
            ],
        )
        .await
        .unwrap();
    assert!(matches!(deleted.outcome, CompoundOutcome::Applied { .. }));
    assert!(storage.get_object::<Counter>(&2, ObjectVersion::Latest).await.unwrap().is_none());
    let point = ObjectPoint {
        schema: second.schema.id,
        item: second.item,
        version: ObjectVersion::Latest,
    };
    assert_eq!(primary.get_object(&after, &point).await.unwrap().unwrap().value.payload, compound_value(2, 30).payload);
    assert!(matches!(
        storage.compare_exchange(None, &Counter { id: 2, value: 40 }).await.unwrap(),
        harmos_storage::CompareExchange::Applied(_)
    ));
    assert_eq!(storage.get_object::<Counter>(&2, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 40);
    // Resolve Repeated Keys in Item Order.
    let third = compound_value(3, 0);
    let repeated = storage
        .mutate_once(
            &MutationId {
                window: mutation.window.clone(),
                key: Identifier::new("repeated-key").unwrap(),
            },
            vec![
                MutationItem::Create(compound_value(3, 50)),
                MutationItem::Delete {
                    schema: third.schema.id,
                    item: third.item,
                    expected: None,
                },
                MutationItem::Create(compound_value(3, 60)),
            ],
        )
        .await
        .unwrap();
    assert!(matches!(repeated.outcome, CompoundOutcome::Applied { .. }));
    assert_eq!(storage.get_object::<Counter>(&3, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 60);

    primary.delete_snapshot(&after).await.unwrap();
    primary.delete_snapshot(&before).await.unwrap();
    storage.close().await.unwrap();
}
