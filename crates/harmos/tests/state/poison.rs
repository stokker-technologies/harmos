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
// Third Party Imports
use support::{Change, Copied, Ledger, Noted, alice, booted, deposit, ledger, quietly};
// Local Imports
use crate::support;
use harmos::{Error, state::Focus, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn poison_refuses_verbs() {
    let runtime = booted(Ledger::default(), |builder| builder).await;
    runtime.state.commit(deposit(40)).await.expect("accepted");
    quietly(runtime.state.commit(Change::Explode))
        .await
        .expect_err("the change panicked");

    let read = runtime.state.state().read(|ledger| ledger.clone()).await;
    let commit = runtime.state.commit(deposit(1)).await;
    let record = runtime.records.publish(Noted("after".to_owned())).await;
    let undo = runtime.state.undo(alice(), Focus::all()).await;
    let query: Vec<_> = runtime
        .state
        .query::<Change>(Position::ORIGIN)
        .await
        .collect();

    assert!(matches!(read, Err(Error::Unavailable)));
    assert!(matches!(commit, Err(Error::Unavailable)));
    assert!(matches!(record, Err(Error::Unavailable)));
    assert!(matches!(undo, Err(Error::Unavailable)));
    assert!(
        matches!(query.as_slice(), [Err(Error::Unavailable)]),
        "no reader observes half-mutated state, not even a historical one"
    );

    // The writer is never restarted in place: a second attempt is refused too,
    // and the runtime stops at the frontier it reached.
    assert!(matches!(
        runtime.state.commit(deposit(2)).await,
        Err(Error::Unavailable)
    ));
    assert_eq!(runtime.stop().await.unwrap(), Position(1));
}

#[tokio::test]
async fn next_boot_recovers() {
    let (copy, stored) = Copied::new();
    let live = booted(Ledger::default(), |builder| builder.recover(copy)).await;

    live.state.commit(deposit(40)).await.expect("accepted");
    live.records
        .publish(Noted("audited".to_owned()))
        .await
        .expect("facts cannot be refused");
    let kept = ledger(&live).await;

    let poisoned = quietly(live.state.commit(Change::Explode)).await;
    let observer = live.state.clone();
    let settled = live.stop().await.unwrap();

    assert!(matches!(poisoned, Err(Error::Unavailable)));
    assert_eq!(
        settled,
        Position(2),
        "the intact tail drained on the way out"
    );
    assert_eq!(
        observer.stored(),
        observer.applied(),
        "evacuation drained until durability caught up with what was applied"
    );

    let recovered = booted(Ledger::default(), |builder| {
        builder.recover(Copied::resuming(&stored))
    })
    .await;

    assert_eq!(
        ledger(&recovered).await,
        kept,
        "the next boot recovered every committed transition that preceded the panic"
    );
    assert_eq!(
        recovered.state.applied(),
        Position(2),
        "and nothing was replayed for the change that panicked"
    );
}

/* === Tests === */
