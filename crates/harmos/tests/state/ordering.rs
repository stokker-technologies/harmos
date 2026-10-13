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
use std::task::Poll;
use std::time::Duration;
// Third Party Imports
use futures_core::Stream;
use support::{Change, Deposit, deposit, runtime};
// Local Imports
use crate::support;
use harmos::{state::Entry, state::Position};

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
        scenario_runtime::check("ac003_publish_precedes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut feed = Box::pin(runtime.state.watch::<Change>(Position::ORIGIN));

                    let receipt = runtime
                        .state
                        .commit(deposit(10))
                        .await
                        .expect("a deposit of 10 is accepted");

                    // Nothing is awaited between the reply arriving and this line, so the
                    // frontier cannot have moved since: it was already published when the
                    // committer heard about the entry.
                    assert_eq!(runtime.state.applied(), receipt.position);

                    // And the feed already has it, on the watcher's very first poll: the tail
                    // append and the doorbell both happened before the reply was sent.
                    let published = std::future::poll_fn(|context| {
                        Poll::Ready(feed.as_mut().poll_next(context))
                    })
                    .await;

                    let Poll::Ready(Some(Ok(entry))) = published else {
                        panic!(
                            "the entry was not published before the receipt arrived: {published:?}"
                        );
                    };
                    assert_eq!(entry.position, receipt.position);
                });
        });
    }
    {
        scenario_runtime::check("journal_ac003_positions", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let journal = &runtime.state;

                    // `join!` polls in declaration order, and a commit's first await is its
                    // channel send, so declaration order is arrival order. The channel holds
                    // far more than three requests, so no send ever parks and reorders.
                    let (first, second, third) = tokio::join!(
                        journal.commit(deposit(1)),
                        journal.commit(deposit(2)),
                        journal.commit(deposit(3)),
                    );
                    let receipts =
                        [first, second, third].map(|sent| sent.expect("accepted").position);

                    assert_eq!(receipts, [Position(1), Position(2), Position(3)]);
                    assert_eq!(
                        amounts(&runtime).await,
                        vec![(Position(1), 1), (Position(2), 2), (Position(3), 3)]
                    );
                });
        });
    }
    {
        scenario_runtime::check("consumes_lacks_position", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;

                    runtime.state.commit(deposit(5)).await.expect("accepted");
                    runtime
                        .state
                        .commit(support::withdraw(900))
                        .await
                        .expect_err("refused");
                    let after = runtime.state.commit(deposit(6)).await.expect("accepted");

                    assert_eq!(
                        after.position,
                        Position(2),
                        "positions are assigned at seal, so a refusal skips no number"
                    );
                });
        });
    }
    {
        scenario_runtime::check("memory_durability_past", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let receipt = runtime.state.commit(deposit(10)).await.expect("accepted");

                    assert_eq!(
                        runtime.state.stored(),
                        runtime.state.applied(),
                        "with nothing attached, stored tracks applied"
                    );

                    // A zero-length timeout polls the inner future once before its deadline
                    // fires, so completing under it means the gate resolved on the first poll —
                    // the same gate, and the same code path, a persisted runtime parks on.
                    let gated = tokio::time::timeout(
                        Duration::ZERO,
                        runtime.state.wait_until_stored(receipt.position),
                    )
                    .await;

                    assert!(gated.is_ok(), "memory-only durability never parks");
                });
        });
    }
}

async fn amounts(runtime: &harmos::Runtime<support::Bank>) -> Vec<(Position, i64)> {
    runtime
        .state
        .query::<Change>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("the tail holds the whole history"))
        .map(
            |Entry {
                 position, value, ..
             }| match value {
                Change::Deposit(Deposit { amount, .. }) => (position, amount),
                other => panic!("history holds only deposits here, found {other:?}"),
            },
        )
        .collect()
}

/* === Tests === */
