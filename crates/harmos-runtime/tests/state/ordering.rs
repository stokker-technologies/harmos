/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    borrow::Borrow,
    sync::{Arc, Weak},
    task::Poll,
};
// Third Party Imports
use futures_core::Stream;
use futures_util::StreamExt;
use support::{Change, Deposit, deposit, runtime};
// Local Imports
use crate::state::support;
use harmos_runtime::{state::Entry, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct OwnedChange {
    transaction: Change,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Borrow<Change> for OwnedChange {
    fn borrow(&self) -> &Change {
        &self.transaction
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn owned(change: Change) -> (harmos_runtime::operations::Response<Change>, Weak<OwnedChange>) {
    let owner = Arc::new(OwnedChange { transaction: change });
    let weak = Arc::downgrade(&owner);
    (harmos_runtime::operations::Response::from_owner(owner), weak)
}

#[tokio::test]
async fn owned_commit_survives_queued_caller_cancellation_and_tail_eviction() {
    let runtime = support::booted(support::Ledger::default(), |setup| {
        setup.settings().tail(1);
    })
    .await;
    let (transaction, owner) = owned(deposit(4));
    let mut commit = Box::pin(runtime.state.commit_owned(transaction));
    let queued = std::future::poll_fn(|context| Poll::Ready(commit.as_mut().poll(context))).await;
    assert!(queued.is_pending());
    drop(commit);
    assert!(owner.upgrade().is_some());

    let mut feed = Box::pin(runtime.state.watch::<harmos_runtime::state::Recorded<support::Bank>>(Position::ORIGIN));
    let entry = feed.next().await.unwrap().unwrap();
    let harmos_runtime::state::Recorded::Transaction(transaction) = &entry.value else {
        panic!("expected transaction");
    };
    let original = owner.upgrade().unwrap();
    assert!(std::ptr::eq(transaction.as_ref(), &original.transaction));
    drop(original);
    runtime.state.commit(deposit(1)).await.unwrap();
    assert!(owner.upgrade().is_some());

    drop(entry);
    drop(feed);
    assert!(owner.upgrade().is_none());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn owned_forward_history_survives_undo_and_redo() {
    let runtime = support::booted(support::Ledger::default(), |setup| {
        setup.settings().tail(1);
    })
    .await;
    let journal = runtime.state.as_principal(support::alice());
    let (transaction, owner) = owned(deposit(4));
    journal.commit_owned(transaction).await.unwrap();
    runtime.state.commit(deposit(1)).await.unwrap();
    assert!(owner.upgrade().is_some());

    journal.undo(support::alice(), harmos_runtime::state::Focus::all()).await.unwrap();
    assert!(owner.upgrade().is_some());
    journal.redo(support::alice(), harmos_runtime::state::Focus::all()).await.unwrap();
    assert!(owner.upgrade().is_some());
    assert_eq!(support::balance(&runtime, support::HOUSE).await, 5);

    journal.undo(support::alice(), harmos_runtime::state::Focus::all()).await.unwrap();
    journal.commit(deposit(2)).await.unwrap();
    assert!(owner.upgrade().is_none());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn keyed_owned_duplicate_releases_the_unused_owner() {
    let runtime = runtime().await;
    let key = harmos_runtime::state::RequestKey::new("owned.duplicate");
    let (first, original) = owned(deposit(4));
    let receipt = runtime.state.commit_keyed_owned(key.clone(), first).await.unwrap();
    let (retry, unused) = owned(deposit(99));
    let repeated = runtime.state.commit_keyed_owned(key, retry).await.unwrap();
    assert_eq!(receipt, repeated);
    assert_eq!(support::balance(&runtime, support::HOUSE).await, 4);
    assert!(original.upgrade().is_some());
    assert!(unused.upgrade().is_none());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn publish_precedes_receipt() {
    let runtime = runtime().await;
    let mut feed = Box::pin(runtime.state.watch::<Change>(Position::ORIGIN));

    let receipt = runtime.state.commit(deposit(10)).await.expect("a deposit of 10 is accepted");

    // Nothing is awaited between the reply arriving and this line, so the
    // frontier cannot have moved since: it was already published when the
    // committer heard about the entry.
    assert_eq!(runtime.state.applied(), receipt.position);

    // And the feed already has it, on the watcher's very first poll: the tail
    // append and the doorbell both happened before the reply was sent.
    let published = std::future::poll_fn(|context| Poll::Ready(feed.as_mut().poll_next(context))).await;

    let Poll::Ready(Some(Ok(entry))) = published else {
        panic!("the entry was not published before the receipt arrived: {published:?}");
    };
    assert_eq!(entry.position, receipt.position);
}

#[tokio::test]
async fn positions_follow_arrival() {
    let runtime = runtime().await;
    let journal = &runtime.state;

    // `join!` polls in declaration order, and a commit's first await is its
    // channel send, so declaration order is arrival order. The channel holds
    // far more than three requests, so no send ever parks and reorders.
    let (first, second, third) = tokio::join!(journal.commit(deposit(1)), journal.commit(deposit(2)), journal.commit(deposit(3)),);
    let receipts = [first, second, third].map(|sent| sent.expect("accepted").position);

    assert_eq!(receipts, [Position(1), Position(2), Position(3)]);
    assert_eq!(amounts(&runtime).await, vec![(Position(1), 1), (Position(2), 2), (Position(3), 3)]);
}

#[tokio::test]
async fn refusal_skips_position() {
    let runtime = runtime().await;

    runtime.state.commit(deposit(5)).await.expect("accepted");
    runtime.state.commit(support::withdraw(900)).await.expect_err("refused");
    let after = runtime.state.commit(deposit(6)).await.expect("accepted");

    assert_eq!(after.position, Position(2), "positions are assigned at seal, so a refusal skips no number");
}

#[tokio::test]
async fn memory_never_parks() {
    let runtime = runtime().await;
    let receipt = runtime.state.commit(deposit(10)).await.expect("accepted");

    assert_eq!(runtime.state.stored(), runtime.state.applied(), "with nothing attached, stored tracks applied");

    // One poll resolving means the gate opened at once: the same gate, and
    // the same code path, a persisted runtime parks on.
    let mut waiting = Box::pin(runtime.state.wait_until_stored(receipt.position));
    let gated = std::future::poll_fn(|context| Poll::Ready(waiting.as_mut().poll(context))).await;

    assert!(gated.is_ready(), "memory-only durability never parks");
}

async fn amounts(runtime: &harmos_runtime::Runtime<support::Bank>) -> Vec<(Position, i64)> {
    runtime
        .state
        .query::<Change>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("the tail holds the whole history"))
        .map(|Entry { position, value, .. }| match value {
            Change::Deposit(Deposit { amount, .. }) => (position, amount),
            other => panic!("history holds only deposits here, found {other:?}"),
        })
        .collect()
}

/* === Tests === */
