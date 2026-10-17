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
use support::{Bank, Ledger, Payouts, booted, deposit, ledger, pay};
// Local Imports
use super::support;
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
async fn unacked_item_redelivered() {
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

/* === Tests === */
