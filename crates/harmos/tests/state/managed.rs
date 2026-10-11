/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::support::{Bank, HOUSE, alice, deposit, runtime};
use harmos::{Application, Focus, ManagedState, Never, Position, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct State {
    value: i64,
}
struct Plain;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Application for Plain {
    type State = State;
    type Transaction = Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("cloned_read_handles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
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
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("state_survives_stop", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let manager = runtime.state.clone();
                    let state = manager.state();
                    manager.commit(deposit(7)).await.unwrap();
                    let through = runtime.stop().await.unwrap();
                    assert_eq!(through, Position(1));
                    assert!(manager.commit(deposit(1)).await.is_err());
                    drop((manager, runtime));
                    let value = state.read(|state| state.balance(HOUSE)).await.unwrap();
                    assert_eq!(value.value, 7);
                    assert_eq!(value.position, through);
                });
        });
    }
    {
        scenario_runtime::check("handles_share_nonclone", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Plain>::builder(State { value: 19 }, ())
                        .runtime()
                        .await
                        .unwrap();
                    let manager = runtime.state.clone();
                    let state = manager.state().clone();
                    let value = state.read(|state| state.value).await.unwrap();
                    assert_eq!(value.value, 19);
                    assert_eq!(value.position, Position::ORIGIN);
                    runtime.stop().await.unwrap();
                });
        });
    }
}
