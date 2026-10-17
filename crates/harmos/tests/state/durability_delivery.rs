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
use std::sync::atomic::Ordering;
// Third Party Imports
use support::{Bank, Explode, Ledger, Payouts, booted, deposit, ledger, pay};
// Local Imports
use super::support;
use harmos::{Error, state::Position};
use support::journal::{Journal, Store};

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
async fn unacked_item_comes() {
    let copy = Store::default();
    let (listener, watched) = Payouts::new();

    let live = booted(Ledger::default(), |builder| {
        builder
            .recover(Journal::<Bank>::open(&copy))
            .attach(listener.holding(7))
    })
    .await;
    live.state.commit(deposit(100)).await.expect("accepted");
    live.state.commit(pay(1, 10)).await.expect("accepted");
    live.state.commit(pay(7, 10)).await.expect("accepted");
    live.stop().await.unwrap();

    let (checkpoint, dispatched) = &watched;
    assert_eq!(
        *dispatched.lock().expect("the listener is never poisoned"),
        vec![1],
        "the held payout was never dispatched"
    );
    assert_eq!(
        checkpoint.load(Ordering::Acquire),
        2,
        "a checkpoint never advances past an item the handler did not acknowledge"
    );

    // The restart: the same persisted checkpoint, and a handler that will
    // acknowledge this time.
    let recovered = booted(Ledger::default(), |builder| {
        builder
            .recover(Journal::<Bank>::open(&copy))
            .attach(Payouts::resuming(&watched))
    })
    .await;
    let restored = ledger(&recovered).await;
    recovered.stop().await.unwrap();

    assert_eq!(
        *dispatched.lock().expect("the listener is never poisoned"),
        vec![1, 7],
        "the unacknowledged item came back under the id its own payload minted"
    );
    assert_eq!(checkpoint.load(Ordering::Acquire), 3);
    assert_eq!(
        restored.payouts,
        vec![1, 7],
        "pending work is durable state, so recovery rebuilt it with everything else"
    );
}

#[tokio::test]
async fn poisoned_writer_evacuates() {
    let copy = Store::default();

    let live = booted(Ledger::default(), |builder| {
        builder.recover(Journal::<Bank>::open(&copy))
    })
    .await;
    live.state.commit(deposit(60)).await.expect("accepted");
    live.state.commit(pay(3, 10)).await.expect("accepted");

    let reporting = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let poisoned = live.state.commit(Explode).await;
    std::panic::set_hook(reporting);

    assert!(matches!(poisoned, Err(Error::Unavailable)));
    assert_eq!(
        live.state.applied(),
        Position(2),
        "no entry was sealed for the change that panicked"
    );
    assert!(matches!(
        live.state.state().read(Clone::clone).await,
        Err(Error::Unavailable)
    ));

    // The writer is gone, so the runtime evacuates without being asked: the
    // folds take the intact tail out, and the process stops rather than
    // restarting anything.
    let observer = live.state.clone();
    let settled = live.stop().await.unwrap();

    assert_eq!(settled, Position(2));
    assert_eq!(
        observer.stored(),
        observer.applied(),
        "evacuation drained until the stored frontier caught the applied one"
    );

    let recovered = booted(Ledger::default(), |builder| {
        builder.recover(Journal::<Bank>::open(&copy))
    })
    .await;

    assert_eq!(
        ledger(&recovered).await,
        Ledger {
            balance: 50,
            applications: 2,
            payouts: vec![3],
        },
        "the next boot recovered every committed transition that preceded the panic"
    );
}

/* === Tests === */
