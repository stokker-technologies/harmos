/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runtime Backend Selection

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use crate::storage::{Backend, Bank, Ledger, RecoveryBackend, StorageBackends, StorageManager, catalog, config, history, write};
use crate::support::{HOUSE, clock::Controlled, deposit};
use harmos_runtime::config::ManagedStorageSettings;
use harmos_runtime::storage::ReadLimits;

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
async fn registered_backends_replace_the_default_and_recover_their_state() {
    // Register Product-Owned Backend Instances Before Startup
    let clock = Controlled::new();
    let data = Arc::new(Backend::new(history()));
    let recovery = Arc::new(RecoveryBackend::new(history()));
    let storage = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        config(),
    )
    .unwrap();
    let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().clock(clock.source()).storage(&storage).register(catalog()).managed_storage(ManagedStorageSettings {
            replay: ReadLimits {
                rows: NonZeroUsize::new(16).unwrap(),
                bytes: NonZeroUsize::new(65536).unwrap(),
            },
            capture: ReadLimits {
                rows: NonZeroUsize::new(16).unwrap(),
                bytes: NonZeroUsize::new(65536).unwrap(),
            },
            snapshots: None,
            journal_tail: 2,
        });
    })
    .await
    .unwrap();

    // Both Writes Reach the Registered Owners
    write(runtime.storage().unwrap(), "product-record").await;
    let receipt = runtime.state.commit(deposit(11)).await.unwrap();
    runtime.storage().unwrap().flush().await.unwrap();
    runtime.storage().unwrap().flush_recovery().await.unwrap();
    assert_eq!(data.records.lock().unwrap().len(), 1);
    assert_eq!(data.saved.load(Ordering::SeqCst), 1);
    assert_eq!(recovery.saved.load(Ordering::SeqCst) as u64, receipt.position.0);
    crate::support::app::shutdown(&runtime).await.unwrap();

    // Reopen Those Owners and Recover the Managed State
    let reopened = StorageManager::new(StorageBackends { primary: data, state: recovery }, config()).unwrap();
    let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().clock(clock.source()).storage(&reopened).register(catalog()).managed_storage(ManagedStorageSettings {
            replay: ReadLimits {
                rows: NonZeroUsize::new(16).unwrap(),
                bytes: NonZeroUsize::new(65536).unwrap(),
            },
            capture: ReadLimits {
                rows: NonZeroUsize::new(16).unwrap(),
                bytes: NonZeroUsize::new(65536).unwrap(),
            },
            snapshots: None,
            journal_tail: 2,
        });
    })
    .await
    .unwrap();
    let state = runtime.state.state().read(|state| state.balances[&HOUSE]).await.unwrap();
    assert_eq!(state.value, 11);
    assert_eq!(runtime.state.applied(), receipt.position);
    assert_eq!(runtime.storage().unwrap().get_saved_position().sequence, 1);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
