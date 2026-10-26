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
use std::sync::{Arc, Mutex};
// Third Party Imports
use support::{Bank, Ledger, Noted, alice, booted, deposit, ledger, withdraw};
// Local Imports
use super::support;
use harmos_runtime::state::{Kind, Position};
use support::journal::{Journal, Snapshot, Store, frame};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Saved {
    at: Position,
    state: Ledger,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn restart_recovers_history() {
    let copy = Store::default();

    let live = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;
    let handle = live.state.as_principal(alice());
    handle.commit(deposit(40)).await.expect("accepted");
    handle.commit(withdraw(15)).await.expect("accepted");
    live.records.publish(Noted("audited".to_owned())).await.expect("facts cannot be refused");
    handle.commit(deposit(2)).await.expect("accepted");
    let expected = ledger(&live).await;
    let applied = live.state.applied();

    // A derived handle is a handle: the writer stops when the last one goes.
    drop(handle);

    assert_eq!(live.stop().await.unwrap(), applied, "shutdown drained the whole tail");

    let recovered = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;

    assert_eq!(ledger(&recovered).await, expected);
    assert_eq!(recovered.state.applied(), applied);
    assert_eq!(expected.balance, 27);
    assert_eq!(expected.applications, 3, "the fact took a position and applied nothing");
}

#[tokio::test]
async fn crash_loses_unsynced() {
    let copy = Store::default();

    // Three entries to a batch, so the fourth sits in the adapter's own risk
    // window when the process dies.
    let live = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy).syncing_every(3))).await;
    for amount in [10, 20, 30, 40] {
        live.state.commit(deposit(amount)).await.expect("accepted");
    }
    live.state.wait_until_stored(Position(3)).await;

    assert_eq!(live.state.applied(), Position(4));
    assert_eq!(live.state.stored(), Position(3), "the fourth entry is inside the window, and nothing left the process on it");

    // The crash: the process dies with its unsynced lines still in memory.
    // Forgotten rather than stopped, because a stop would flush the held
    // batch — a crash is exactly the path where nobody gets to.
    std::mem::forget(live);

    let recovered = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;

    assert_eq!(ledger(&recovered).await.balance, 60, "everything synced survived, and only the window was lost");
    assert_eq!(recovered.state.applied(), Position(3));
}

#[tokio::test]
async fn redelivery_forwards_stored() {
    let copy = Store::default();
    let mirror = Store::default();
    let second_mirror = Store::default();

    // One entry as a version-1 build stored it, written valid but
    // non-canonical — a member order and spacing no serializer of this build
    // produces — so any re-encoding, of the payload or of the frame, fails
    // the byte comparison below.
    let historical = r#"[{"kind":"Transaction","position":9,"id":"ledger.bonus","version":1,"metadata":{"principal":null,"correlation":9,"causation":null,"request":null,"link":null,"at":{"secs_since_epoch":0,"nanos_since_epoch":0}},"payload":{ "amount":3 }}]"#;
    // And one record this build has never heard of: it is skipped by the
    // fold, yet its line must still reach a durable copy whole.
    let unreadable = frame(10, Kind::Record, "ledger.audited", 1, serde_json::json!({ "note": "kept" }));

    seeded(&copy).await;
    copy.push(historical);
    copy.push(unreadable.clone());

    // A fresh mirror lags behind everything, so boot redelivers it the whole
    // recovered history.
    let recovered = booted(Ledger::default(), |builder| {
        builder.recover(Journal::<Bank>::open(&copy)).attach(Journal::<Bank>::open(&mirror)).attach(Journal::<Bank>::open(&second_mirror))
    })
    .await;

    assert_eq!(ledger(&recovered).await.balance, 16, "the version-1 bonus still applies through its own version");
    assert_eq!(recovered.state.applied(), Position(10), "the unknown record was skipped, and its position was taken all the same");
    recovered.stop().await.unwrap();

    let written = mirror.lines();

    assert_eq!(
        written,
        copy.lines(),
        "a lagging durable copy is redelivered history's own bytes, so the \
         mirror equals its source frame for frame"
    );
    assert!(written.iter().any(|line| line == historical), "the historical entry arrived verbatim, spacing and member order intact");
    assert!(written.contains(&unreadable), "a record this build cannot decode is still forwarded, not silently dropped");

    let source_allocations = copy.shared_lines();
    let first_allocations = mirror.shared_lines();
    let second_allocations = second_mirror.shared_lines();
    assert_eq!(source_allocations.len(), first_allocations.len());
    assert_eq!(source_allocations.len(), second_allocations.len());
    for ((source, first), second) in source_allocations.iter().zip(&first_allocations).zip(&second_allocations) {
        assert!(Arc::ptr_eq(source, first));
        assert!(Arc::ptr_eq(source, second));
    }
    drop((copy, mirror, recovered, source_allocations, first_allocations, second_allocations));
    assert_eq!(second_mirror.lines(), written);
}

#[tokio::test]
async fn undecodable_history_forwarded() {
    let copy = Store::default();
    let mirror = Store::default();

    // More lines than the redelivery window holds of entries, and not one of
    // them decodable by this build: the byte lane is the only way any of them
    // travels, so a window on that lane would silently amputate the copy.
    let history: Vec<String> =
        (1..=1030).map(|position| frame(position, Kind::Record, "ledger.audited", 1, serde_json::json!({ "note": "kept" }))).collect();
    copy.replace(history.clone());

    let recovered =
        booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy)).attach(Journal::<Bank>::open(&mirror))).await;

    assert_eq!(recovered.state.applied(), Position(1030), "every skipped record's position was taken all the same");
    recovered.stop().await.unwrap();

    assert_eq!(
        mirror.lines(),
        history,
        "a history of records only a later build can read reaches a fresh \
         mirror whole and byte-identical, with no window shortening it"
    );
}

#[tokio::test]
async fn stop_flushes_batch() {
    let copy = Store::default();

    // Three entries to a batch, so both commits sit in the adapter's own risk
    // window when the stop begins.
    let live = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy).syncing_every(3))).await;
    for amount in [10, 20] {
        live.state.commit(deposit(amount)).await.expect("accepted");
    }
    let settled = live.stop().await.unwrap();

    // The settled frontier is read back after the flush; the drain alone had
    // synced nothing under a three-entry batch.
    assert_eq!(settled, Position(2), "stop reports what the flush made durable, not what the drain had synced");

    let recovered = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy))).await;

    assert_eq!(ledger(&recovered).await.balance, 30, "the held batch reached disk on stop rather than dying with the process");
    assert_eq!(recovered.state.applied(), Position(2));
}

#[tokio::test]
async fn stored_resolves_sync() {
    let copy = Store::default();
    let runtime = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(&copy).syncing_every(2))).await;

    let first = runtime.state.commit(deposit(5)).await.expect("accepted");
    let mut waiting = Box::pin(runtime.state.wait_until_stored(first.position));
    tokio::task::yield_now().await;
    let parked = std::future::poll_fn(|context| std::task::Poll::Ready(waiting.as_mut().poll(context).is_pending())).await;

    assert!(parked, "the entry is applied but not yet acknowledged by the copy");

    runtime.state.commit(deposit(6)).await.expect("accepted");
    waiting.await;
    runtime.state.wait_until_stored(Position(2)).await;

    assert_eq!(runtime.state.stored(), runtime.state.applied());
}

#[tokio::test]
async fn tail_restores_state() {
    let copy = Store::default();
    let snapshot: Arc<Mutex<Option<Saved>>> = Arc::default();

    let live = booted(Ledger::default(), |builder| {
        builder.recover(Journal::<Bank>::open(&copy)).snapshot(Snapshot::<Bank>::folding(Ledger::default(), Position::ORIGIN, {
            let snapshot = Arc::clone(&snapshot);
            move |state, at| {
                let saved = Saved { at, state: state.clone() };
                *snapshot.lock().expect("the snapshot is never poisoned") = Some(saved);

                Ok(())
            }
        }))
    })
    .await;
    for amount in [10, 20, 30, 40, 50] {
        live.state.commit(deposit(amount)).await.expect("accepted");
    }
    let expected = ledger(&live).await;
    let applied = live.state.applied();
    live.stop().await.unwrap();

    // Boot the same equation the other way round: the origin is the snapshot,
    // and the entries above its horizon are the rest.
    let saved = snapshot.lock().expect("the snapshot is never poisoned").take().expect("a snapshot was taken");
    let recovered = booted(saved.state, |builder| builder.recover(Journal::<Bank>::open(&copy).after(saved.at))).await;

    assert_eq!(saved.at, applied, "the snapshot names the position it holds");
    assert_eq!(ledger(&recovered).await, expected);
    assert_eq!(recovered.state.applied(), applied, "an empty tail above the horizon still stands at the snapshot's position");
}

async fn seeded(copy: &Store) {
    let runtime = booted(Ledger::default(), |builder| builder.recover(Journal::<Bank>::open(copy))).await;

    for amount in [1, 2, 3, 4] {
        runtime.state.commit(deposit(amount * 2)).await.expect("accepted");
        runtime.state.commit(withdraw(amount)).await.expect("accepted");
    }
    runtime.stop().await.unwrap();
}

/* === Tests === */
