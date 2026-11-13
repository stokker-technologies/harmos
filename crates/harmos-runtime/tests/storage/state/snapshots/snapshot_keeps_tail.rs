/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test]
async fn snapshot_keeps_tail() {
    let store = Store::new();

    let storage = store.manager(config());
    let runtime = crate::support::app::launch::<Model>(0, (), |setup| {
        setup.settings().storage(&storage).register(transactions_state_catalog()).register(records_registry()).managed_storage(
            harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: None,
                journal_tail: 1,
            },
        );
    })
    .await
    .unwrap();
    runtime.state.commit(Add { amount: 3 }).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    let storage = store.manager(config());
    let runtime = crate::support::app::launch::<Model>(0, (), |setup| {
        setup.settings().storage(&storage).register(transactions_state_catalog()).register(records_registry()).managed_storage(
            harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: Some(policy(100)),
                journal_tail: 1,
            },
        );
    })
    .await
    .unwrap();
    assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 3);
    crate::support::app::shutdown(&runtime).await.unwrap();
    let snapshot: serde_json::Value = serde_json::from_reader(store.snapshot().payload.reader().unwrap()).unwrap();
    assert_eq!(snapshot["tail"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot["tail"][0]["version"], Add::ENTRY.definition().1);
    store.recovery.entries.lock().unwrap().clear();
    let storage = store.manager(config());
    let runtime = crate::support::app::launch::<Model>(0, (), |setup| {
        setup.settings().storage(&storage).register(transactions_state_catalog()).register(records_registry()).managed_storage(
            harmos_runtime::config::ManagedStorageSettings {
                replay: limits(),
                capture: limits(),
                snapshots: Some(policy(100)),
                journal_tail: 1,
            },
        );
    })
    .await
    .unwrap();
    assert_eq!(runtime.state.state().read(|state| *state).await.unwrap().value, 3);
    let facts: Vec<_> = runtime.state.query::<Records>(Position::ORIGIN).await.collect();
    assert_eq!(facts.len(), 1);
    assert!(facts[0].is_ok());
    crate::support::app::shutdown(&runtime).await.unwrap();
}
