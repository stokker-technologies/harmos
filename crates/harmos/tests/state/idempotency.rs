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
use support::{Change, HOUSE, balance, deposit, runtime};
// Local Imports
use crate::support;
use harmos::{Entry, Position, RequestKey};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("journal_ac006_retried", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let key = RequestKey::new("transfer-9");

                    let first = runtime
                        .state
                        .commit_keyed(key.clone(), deposit(25))
                        .await
                        .expect("accepted");
                    let retry = runtime
                        .state
                        .commit_keyed(key.clone(), deposit(25))
                        .await
                        .expect("the retry answers with what the first attempt got");

                    assert_eq!(retry, first);
                    assert_eq!(
                        runtime.state.applied(),
                        first.position,
                        "the retry minted nothing"
                    );
                    assert_eq!(
                        balance(&runtime, HOUSE).await,
                        25,
                        "the retry applied nothing"
                    );

                    let history = history(&runtime).await;
                    assert_eq!(history.len(), 1, "the retry appended nothing");
                    assert_eq!(history[0].metadata.request, Some(key));
                });
        });
    }
    {
        scenario_runtime::check("journal_ac006_second", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;

                    let first = runtime
                        .state
                        .commit_keyed(RequestKey::new("one"), deposit(25))
                        .await
                        .expect("accepted");
                    let second = runtime
                        .state
                        .commit_keyed(RequestKey::new("two"), deposit(25))
                        .await
                        .expect("accepted");

                    assert_eq!(first.position, Position(1));
                    assert_eq!(second.position, Position(2));
                    assert_eq!(balance(&runtime, HOUSE).await, 50);
                });
        });
    }
}

async fn history(runtime: &harmos::Runtime<support::Bank>) -> Vec<Entry<Change>> {
    runtime
        .state
        .query::<Change>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("the tail holds the whole history"))
        .collect()
}

/* === Tests === */
