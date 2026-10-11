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
use support::{Bank, Change, Ledger, alice, booted, deposit, scratch};
// Local Imports
use super::support;
use harmos::{Error, Focus, Origin, Position, Runtime};
use jsonl_adapter::Jsonl;

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
        scenario_runtime::check("state_matches_replay", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("history-state");
                    let copy = directory.join("journal.jsonl");
                    let runtime = booted(Ledger::default(), |builder| {
                        builder
                            .recover(Jsonl::<Bank>::open(&copy).expect("a fresh copy"))
                            .tail(1)
                    })
                    .await;
                    let journal = runtime.state.as_principal(alice());
                    let first = journal.commit(deposit(4)).await.expect("accepted");
                    let second = journal.commit(deposit(7)).await.expect("accepted");
                    let undone = journal
                        .undo(alice(), Focus::all())
                        .await
                        .expect("the second deposit is invertible");
                    runtime.state.wait_until_stored(undone.position).await;

                    let at_first = runtime
                        .state
                        .state()
                        .at(Origin::initial(Ledger::default()), first.position)
                        .await
                        .expect("the copy retains the first position");
                    let at_second = runtime
                        .state
                        .state()
                        .at(Origin::initial(Ledger::default()), second.position)
                        .await
                        .expect("the copy retains the second position");
                    let at_undo = runtime
                        .state
                        .state()
                        .at(Origin::initial(Ledger::default()), undone.position)
                        .await
                        .expect("the undo is an ordinary stored entry");

                    assert_eq!(at_first.value.balance, 4);
                    assert_eq!(at_second.value.balance, 11);
                    assert_eq!(at_undo.value.balance, 4);
                    assert_eq!(at_undo.position, undone.position);
                    drop(journal);
                    runtime.stop().await.unwrap();

                    let replayed = booted(Ledger::default(), |builder| {
                        builder.recover(Jsonl::<Bank>::open(&copy).expect("the copy reopens"))
                    })
                    .await;
                    let from_snapshot = replayed
                        .state
                        .state()
                        .at(Origin::at(at_first.value, first.position), undone.position)
                        .await
                        .expect("a caller-loaded snapshot anchors the detached fold");
                    let rebuilt = replayed
                        .state
                        .state()
                        .read(|ledger| Ledger {
                            balance: ledger.balance,
                            applications: ledger.applications,
                            payouts: ledger.payouts.clone(),
                        })
                        .await
                        .expect("boot replay rebuilt the same state");

                    assert_eq!(from_snapshot, rebuilt);
                    replayed.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("source_query_equals", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("history-query");
                    let copy = directory.join("journal.jsonl");
                    let runtime = booted(Ledger::default(), |builder| {
                        builder
                            .recover(Jsonl::<Bank>::open(&copy).expect("a fresh copy"))
                            .tail(2)
                    })
                    .await;

                    let mut expected = Vec::new();
                    let mut after = Position::ORIGIN;
                    for amount in [1, 2, 3, 4] {
                        let receipt = runtime
                            .state
                            .commit(deposit(amount))
                            .await
                            .expect("accepted");
                        runtime.state.wait_until_stored(receipt.position).await;
                        let mut recorded = runtime
                            .state
                            .query::<Change>(after)
                            .await
                            .collect::<Result<Vec<_>, _>>()
                            .expect("the tail records the entry before it can be evicted");
                        expected.append(&mut recorded);
                        after = receipt.position;
                    }
                    let entries: Vec<_> = runtime
                        .state
                        .query::<Change>(Position::ORIGIN)
                        .await
                        .collect::<Result<_, _>>()
                        .expect("the source serves the evicted prefix");

                    assert_eq!(entries, expected);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("refusals_distinguish_origin", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = booted(Ledger::default(), |builder| builder.tail(1)).await;
                    runtime.state.commit(deposit(1)).await.expect("accepted");
                    runtime.state.commit(deposit(2)).await.expect("accepted");

                    let origin_ahead = runtime
                        .state
                        .state()
                        .at(Origin::at(Ledger::default(), Position(2)), Position(1))
                        .await;
                    let sourceless: Vec<_> = runtime
                        .state
                        .query::<Change>(Position::ORIGIN)
                        .await
                        .collect();

                    assert!(matches!(origin_ahead, Err(Error::OriginAhead)));
                    assert!(matches!(sourceless.as_slice(), [Err(Error::BeyondTail)]));

                    let anchor = Origin::at(
                        Ledger {
                            balance: 3,
                            applications: 2,
                            payouts: Vec::new(),
                        },
                        Position(2),
                    );
                    let exact = runtime
                        .state
                        .state()
                        .at(anchor, Position(2))
                        .await
                        .expect("the caller's exact anchor needs no source");
                    assert_eq!(exact.value.balance, 3);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("inspector_recovers_read", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("history-inspect");
                    let copy = directory.join("journal.jsonl");
                    let runtime = booted(Ledger::default(), |builder| {
                        builder.recover(Jsonl::<Bank>::open(&copy).expect("a fresh copy"))
                    })
                    .await;
                    for amount in [2, 5] {
                        let receipt = runtime
                            .state
                            .commit(deposit(amount))
                            .await
                            .expect("accepted");
                        runtime.state.wait_until_stored(receipt.position).await;
                    }
                    runtime.stop().await.unwrap();

                    let inspector = Runtime::<Bank>::inspect(
                        Origin::initial(Ledger::default()),
                        Jsonl::<Bank>::open(&copy).expect("the copy opens read-only"),
                    )
                    .await
                    .expect("the copy recovers without a writer");
                    let current = inspector
                        .state
                        .read(|ledger| ledger.balance)
                        .await
                        .expect("current state is readable");
                    let first = inspector
                        .state
                        .state_at(Origin::initial(Ledger::default()), Position(1))
                        .await
                        .expect("historical state is readable");
                    let entries: Vec<_> = inspector
                        .state
                        .query::<Change>(Position::ORIGIN)
                        .await
                        .collect::<Result<_, _>>()
                        .expect("history is readable");

                    assert_eq!(current.value, 7);
                    assert_eq!(current.position, Position(2));
                    assert_eq!(first.value.balance, 2);
                    assert_eq!(entries.len(), 2);
                    assert_eq!(inspector.state.applied(), Position(2));
                    assert_eq!(inspector.state.stored(), Position(2));
                });
        });
    }
}

/* === Tests === */
