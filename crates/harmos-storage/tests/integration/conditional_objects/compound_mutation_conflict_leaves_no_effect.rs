/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Refuse Every Item on a Conflict

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
use harmos_storage::BackendFailure;
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
async fn compound_mutation_conflict_leaves_no_effect() {
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
    // Seed the Expected Revision.
    let initial = storage.write_object(&Counter { id: 1, value: 10 }).await.unwrap();
    storage.flush_until(&initial.position).await.unwrap();
    let mutation = MutationId {
        window: window.clone(),
        key: Identifier::new("refused").unwrap(),
    };
    let record = fixture::record(&compound_value(1, 0).schema, &Identifier::new("event").unwrap(), &[7]);
    let items = vec![
        MutationItem::Create(compound_value(2, 30)),
        MutationItem::Append(record),
        MutationItem::Set {
            expected: initial.version.clone(),
            value: compound_value(99, 40),
        },
    ];

    // A Late Conflict Must Publish Neither Earlier Item.
    let reply = storage.mutate_once(&mutation, items.clone()).await.unwrap();
    assert_eq!(reply.outcome, CompoundOutcome::Conflict { index: 2, current: None });
    assert!(storage.get_object::<Counter>(&2, ObjectVersion::Latest).await.unwrap().is_none());
    assert!(primary.records.lock().unwrap().is_empty());
    assert_eq!(storage.get_object::<Counter>(&1, ObjectVersion::Latest).await.unwrap().unwrap().value.value, 10);
    assert_eq!(storage.mutate_once(&mutation, items).await.unwrap(), reply);

    // Oversized Atomic Requests Refuse Without Splitting or Admission.
    let accepted = storage.status().accepted;
    let oversized = MutationId {
        window: window.clone(),
        key: Identifier::new("oversized").unwrap(),
    };
    let mut value = compound_value(3, 50);
    value.payload = vec![0; storage.settings().max_batch_bytes().get() + 1];
    assert!(storage.mutate_once(&oversized, vec![MutationItem::Create(value)]).await.is_err());
    assert_eq!(storage.status().accepted, accepted);

    // Capability Refusal Must Happen Before Admission.
    primary.control.unsupported_conditional.store(true, Ordering::Release);
    let accepted = storage.status().accepted;
    let unsupported = MutationId {
        window,
        key: Identifier::new("unsupported").unwrap(),
    };
    let error = storage.mutate_once(&unsupported, vec![MutationItem::Create(compound_value(3, 50))]).await.unwrap_err();
    assert_eq!(error.backend_failure(), Some(BackendFailure::Unsupported));
    assert_eq!(storage.status().accepted, accepted);
    storage.close().await.unwrap();
}
