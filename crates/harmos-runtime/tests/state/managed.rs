/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::state::support::{Bank, HOUSE, alice, deposit, runtime};
use harmos_runtime::{state::Focus, state::ManagedState, state::Position};

/* === Tests === */

#[tokio::test]
async fn cloned_read_handles() {
    let runtime = runtime().await;
    let manager = runtime.state.as_principal(alice());
    let state: ManagedState<Bank> = manager.state();
    let cloned = state.clone();
    let receipt = manager.clone().commit(deposit(40)).await.unwrap();
    let value = cloned.read(|state| state.balance(HOUSE)).await.unwrap();
    assert_eq!(value.value, 40);
    assert_eq!(value.position, receipt.frontier);

    let undone = manager.undo(alice(), Focus::all()).await.unwrap();
    let value = state.read(|state| state.balance(HOUSE)).await.unwrap();
    assert_eq!(value.value, 0);
    assert_eq!(value.position, undone.frontier);

    let redone = manager.redo(alice(), Focus::all()).await.unwrap();
    let value = cloned.read(|state| state.balance(HOUSE)).await.unwrap();
    assert_eq!(value.value, 40);
    assert_eq!(value.position, redone.frontier);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn state_survives_stop() {
    let runtime = runtime().await;
    let manager = runtime.state.clone();
    let state = manager.state();
    manager.commit(deposit(7)).await.unwrap();
    let through = crate::support::app::shutdown(&runtime).await.unwrap();
    assert_eq!(through, Position(1));
    assert!(manager.commit(deposit(1)).await.is_err());
    drop((manager, runtime));
    let value = state.read(|state| state.balance(HOUSE)).await.unwrap();
    assert_eq!(value.value, 7);
    assert_eq!(value.position, through);
}
