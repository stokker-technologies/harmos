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
use std::sync::atomic::Ordering;
// Third Party Imports

// Local Imports
use super::{Echo, publications, runtime};
use crate::support::{DEPOSIT, Deposit, HOUSE};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("provider_transaction_receipt", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = publications::Lifecycle::new(&runtime);
                    let original = lifecycle.state.as_ref().unwrap().clone();
                    let mut manager =
                        publications::Manager::new(lifecycle.candidate("idle").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let payload = serde_json::to_string(&Deposit {
                        account: HOUSE,
                        amount: 7,
                    })
                    .unwrap();
                    let denied = format!("invoke:ledger.withdraw:unselected:{payload}");
                    let refused = runtime
                        .operations
                        .call::<Echo>(Echo {
                            say: denied.clone(),
                        })
                        .await
                        .unwrap();
                    assert!(
                        refused.said.contains("may not invoke operation"),
                        "{}",
                        refused.said
                    );
                    // No Grant Names Transactions: The Provider Refuses The Withdrawal Itself
                    assert!(refused.said.contains("host.invoke"), "{}", refused.said);
                    assert_eq!(original.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(original.handled.load(Ordering::SeqCst), 1);
                    assert_eq!(original.finalized.load(Ordering::SeqCst), 1);

                    let accepted = format!("invoke:{}:same-original-commit:{payload}", DEPOSIT.1);
                    let first = runtime
                        .operations
                        .call::<Echo>(Echo {
                            say: accepted.clone(),
                        })
                        .await
                        .unwrap();
                    let second = runtime
                        .operations
                        .call::<Echo>(Echo { say: accepted })
                        .await
                        .unwrap();
                    assert_eq!(first.said, second.said);
                    let position = first
                        .said
                        .strip_prefix("receipt:")
                        .unwrap()
                        .split(':')
                        .next()
                        .unwrap()
                        .parse::<u64>()
                        .unwrap();
                    assert!(position > 0);
                    assert_eq!(original.prepared.load(Ordering::SeqCst), 3);
                    assert_eq!(original.handled.load(Ordering::SeqCst), 3);
                    assert_eq!(original.finalized.load(Ordering::SeqCst), 3);
                    let state = runtime
                        .state
                        .state()
                        .read(|state| (state.balances.get(&HOUSE).copied(), state.applications))
                        .await
                        .unwrap();
                    assert_eq!(state.value, (Some(7), 1));

                    let malformed = runtime
                        .operations
                        .call::<Echo>(Echo {
                            say: format!("invoke:{}:malformed:invalid-json", DEPOSIT.1),
                        })
                        .await
                        .unwrap();
                    assert!(
                        !malformed.said.starts_with("receipt:"),
                        "{}",
                        malformed.said
                    );
                    assert_eq!(original.prepared.load(Ordering::SeqCst), 4);
                    assert_eq!(original.handled.load(Ordering::SeqCst), 4);
                    assert_eq!(original.finalized.load(Ordering::SeqCst), 4);
                    let refused = runtime
                        .operations
                        .call::<Echo>(Echo { say: denied })
                        .await
                        .unwrap();
                    assert!(
                        refused.said.contains("may not invoke operation"),
                        "{}",
                        refused.said
                    );
                    assert_eq!(original.prepared.load(Ordering::SeqCst), 5);
                    assert_eq!(original.handled.load(Ordering::SeqCst), 5);
                    assert_eq!(original.finalized.load(Ordering::SeqCst), 5);
                    let state = runtime
                        .state
                        .state()
                        .read(|state| (state.balances.get(&HOUSE).copied(), state.applications))
                        .await
                        .unwrap();
                    assert_eq!(state.value, (Some(7), 1));
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
