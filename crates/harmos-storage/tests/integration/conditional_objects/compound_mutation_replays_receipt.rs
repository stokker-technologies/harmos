/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Replay Compound Receipts Across Reopen

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, UNIX_EPOCH};

// Third Party Imports

// Local Imports
use super::{Counter, catalog, compound_value};
use crate::manager::{backend, fixture};
use harmos_storage::MutationValue;
use harmos_storage::{CompoundOutcome, Identifier, MutationId, MutationItem, ObjectVersion, StorageBackends, StorageManager};

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
async fn compound_mutation_replays_receipt() {
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
    // Lose the Reply After the Backend Commits Its Effects and Receipt.
    let mutation = MutationId {
        window,
        key: Identifier::new("lost-reply").unwrap(),
    };
    let record = fixture::record(&compound_value(1, 0).schema, &Identifier::new("event").unwrap(), &[7]);
    let items = vec![MutationItem::Create(compound_value(1, 10)), MutationItem::Append(record)];
    primary.control.compound_lost_replies.store(1, Ordering::Release);
    let first = storage.mutate_once(&mutation, items.clone()).await;
    if first.is_err() {
        storage.retry_transient_persistence();
    }
    let reply = storage.mutate_once(&mutation, items.clone()).await.unwrap();
    assert!(matches!(reply.outcome, CompoundOutcome::Applied { .. }));
    assert_eq!(primary.records.lock().unwrap().len(), 1);
    assert_eq!(primary.objects.lock().unwrap().len(), 1);
    let saved = primary.mutation_receipts.lock().unwrap()[&mutation].clone();
    assert_eq!(saved.value, MutationValue::Compound(reply.clone()));
    assert!(storage.mutate_once(&mutation, vec![MutationItem::Create(compound_value(1, 99))]).await.is_err());
    storage.close().await.unwrap();

    // Reopen the Same Backend and Replay the Original Cut and Outcome.
    let reopened = StorageManager::with_catalog(
        catalog(),
        StorageBackends {
            primary: primary.clone(),
            state: recovery,
        },
        backend::storage_settings(),
    )
    .unwrap();
    reopened.adopt_clock(&clock).unwrap();
    fixture::start(&reopened).await;
    assert_eq!(reopened.mutate_once(&mutation, items).await.unwrap(), reply);
    assert_eq!(primary.records.lock().unwrap().len(), 1);
    assert_eq!(reopened.get_object::<Counter>(&1, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 10);
    reopened.close().await.unwrap();
}
