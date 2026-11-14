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

// Third Party Imports

// Local Imports
use crate::storage::{Bank, Ledger, catalog, write};
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
async fn unconfigured_runtime_saves_data_and_recovery_in_memory() {
    // Start Without a Backend or Storage Configuration
    let clock = Controlled::new();
    let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().clock(clock.source()).register(catalog()).managed_storage(ManagedStorageSettings {
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
    let storage = runtime.storage().unwrap();
    assert!(storage.status().running);

    // Save Primary Data and Managed Recovery
    write(storage, "default-record").await;
    let receipt = runtime.state.commit(deposit(7)).await.unwrap();
    let saved = storage.flush().await.unwrap();
    storage.flush_recovery().await.unwrap();
    assert_eq!(saved.sequence, 1);
    assert_eq!(runtime.state.stored(), receipt.position);
    let state = runtime.state.state().read(|state| state.balances[&HOUSE]).await.unwrap();
    assert_eq!(state.value, 7);

    // A Separate Runtime Starts With Independent Empty Adapters
    let other = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().clock(clock.source()).register(catalog());
    })
    .await
    .unwrap();
    assert_eq!(other.storage().unwrap().status().saved.sequence, 0);
    assert_eq!(other.storage().unwrap().status().recovery_saved.sequence, 0);
    assert_ne!(storage.get_saved_position().history, other.storage().unwrap().get_saved_position().history);
    crate::support::app::shutdown(&other).await.unwrap();
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert!(storage.status().closed);
}
