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
use harmos_storage::*;
use std::{num::NonZeroU32, sync::Arc};

/* === Structs === */

#[derive(harmos_domain::Message, Clone)]
struct Value {
    value: u64,
}

/* === Tests === */

#[tokio::test]
async fn schema_exchange_records_original_outcomes_and_revisions_across_restart() {
    let history = backend::history();
    let settings = backend::storage_settings();
    let (clock, driver) = execution::Execution::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let story = async {
    let primary = Arc::new(memory::Backend::with_execution(ObjectCatalog::new(), history.clone(), clock.clone()));
    let graph = schema::graph::Schema::from_declaration(
        <Value as schema::Described>::DEFINITION,
        &harmos_domain::config::SchemaSettings::default(),
    )
    .unwrap();
    let key = SchemaKey {
        id: Identifier::new("value").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let mut original = None;
    for restart in [false, true] {
        let storage = StorageManager::with_execution(
            StorageBackends {
                primary: primary.clone(),
                state: Arc::new(backend::RecoveryBackend::new(history.clone())),
            },
            settings.clone(),
            clock.clone(),
        )
        .unwrap();
        fixture::start(&storage).await;
        if !restart {
            let window = storage.mutation_window().unwrap().id;
            let a = MutationId {
                window: window.clone(),
                key: Identifier::new("create-a").unwrap(),
            };
            let b = MutationId {
                window: window.clone(),
                key: Identifier::new("create-b").unwrap(),
            };
            let (left, right) = tokio::join!(
                storage.set_schema_once(&a, key.clone(), graph.clone(), None),
                storage.set_schema_once(&b, key.clone(), graph.clone(), None)
            );
            assert_ne!(left.is_ok(), right.is_ok());
            let (winner, loser, first) = if let Ok(first) = left { (a, b, first) } else { (b, a, right.unwrap()) };
            let get = storage.snapshot(&storage.get_saved_position()).await.unwrap().get_schema(&key).await.unwrap().unwrap();
            assert_eq!(get.position, first);
            let update = MutationId {
                window: window.clone(),
                key: Identifier::new("same-value").unwrap(),
            };
            let next = storage.set_schema_once(&update, key.clone(), graph.clone(), Some(first.clone())).await.unwrap();
            assert!(next.sequence > first.sequence);
            let mut different = graph.clone();
            different.definitions[0][0].name = "different".into();
            let bad = MutationId {
                window,
                key: Identifier::new("immutable").unwrap(),
            };
            assert_eq!(
                storage.set_schema_once(&bad, key.clone(), different, Some(next.clone())).await.unwrap_err().backend_failure(),
                Some(BackendFailure::Conflict)
            );
            original = Some((winner, loser, first, next));
        }
        let (winner, loser, first, next) = original.as_ref().unwrap();
        assert_eq!(storage.set_schema_once(winner, key.clone(), graph.clone(), None).await.unwrap(), *first);
        assert_eq!(
            storage.set_schema_once(loser, key.clone(), graph.clone(), None).await.unwrap_err().backend_failure(),
            Some(BackendFailure::Conflict)
        );
        let read = storage.snapshot(&storage.get_saved_position()).await.unwrap();
        assert_eq!(read.get_schema(&key).await.unwrap().unwrap().position, *next);
        drop(read);
        storage.close().await.unwrap();
    }
    };
    tokio::pin!(story);
    loop {
        tokio::select! {
            biased;
            () = &mut story => break,
            () = driver.advance_next() => {}
        }
    }
}
