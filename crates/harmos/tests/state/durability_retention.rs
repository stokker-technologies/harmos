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
use support::{Bank, Change, Ledger, booted, deposit, ledger};
// Local Imports
use super::support;
use harmos::{Error, state::Origin, state::Position, storage::Source};
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
async fn pruned_copy_recovers() {
    let copy = Store::default();
    let runtime = booted(Ledger::default(), |builder| {
        builder.recover(Journal::<Bank>::open(&copy))
    })
    .await;

    let first = runtime.state.commit(deposit(1)).await.expect("accepted");
    let origin = ledger(&runtime).await;
    for amount in [2, 3] {
        runtime
            .state
            .commit(deposit(amount))
            .await
            .expect("accepted");
    }
    runtime.stop().await.unwrap();

    let mut source = Journal::<Bank>::open(&copy);
    Source::prune(&mut source, first.position).await;

    assert_eq!(copy.origin(), first.position);

    let recovered = booted(origin, |builder| {
        builder.recover(Journal::<Bank>::open(&copy))
    })
    .await;
    assert_eq!(ledger(&recovered).await.balance, 6);
    assert!(matches!(
        recovered
            .state
            .query::<Change>(Position::ORIGIN)
            .await
            .next(),
        Some(Err(Error::BeneathHorizon))
    ));
    assert!(matches!(
        recovered
            .state
            .state()
            .at(Origin::initial(Ledger::default()), first.position)
            .await,
        Err(Error::BeneathHorizon)
    ));
    recovered.stop().await.unwrap();
}

/* === Tests === */
