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
use support::{Change, Refusal, VAULT, alice, bob, deposit, deposit_to, ledger, runtime, withdraw};
// Local Imports
use crate::state::support;
use harmos_runtime::{Error, state::At, state::Focus, state::Position, state::Reversals};

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
async fn conflicting_undo_refused() {
    let runtime = runtime().await;
    let mine = runtime.state.as_principal(alice());
    let theirs = runtime.state.as_principal(bob());

    mine.commit(deposit(40)).await.expect("accepted");
    theirs.commit(withdraw(40)).await.expect("the world moves on");

    let before = ledger(&runtime).await;
    let frontier = runtime.state.applied();

    let refused = mine.undo(alice(), Focus::all()).await;

    assert!(matches!(refused, Err(Error::UndoConflict(Refusal::Overdrawn))));
    assert_eq!(runtime.state.applied(), frontier, "a conflicting undo appends nothing");
    assert_eq!(ledger(&runtime).await, before, "the inverse ran its own check before anything mutated");
    assert_eq!(history(&runtime).await, vec![Position(1), Position(2)]);
}

#[tokio::test]
async fn undo_selects_within() {
    let runtime = runtime().await;
    let mine = runtime.state.as_principal(alice());
    let theirs = runtime.state.as_principal(bob());

    mine.commit(deposit_to(VAULT, 7)).await.expect("accepted");
    theirs.commit(deposit(40)).await.expect("accepted");

    // Bob's newer entry neither blocks alice's undo nor is chosen by it.
    mine.undo(alice(), Focus::all()).await.expect("alice's own entry is still the candidate");

    assert_eq!(support::balance(&runtime, VAULT).await, 0);
    assert_eq!(support::balance(&runtime, support::HOUSE).await, 40);
    assert!(
        matches!(mine.undo(alice(), Focus::all()).await, Err(Error::NothingToUndo)),
        "alice cannot reach into bob's history for a second undo"
    );
}

#[tokio::test]
async fn barrier_blocks_undo() {
    let runtime = runtime().await;
    let mine = runtime.state.as_principal(alice());

    mine.commit(deposit(40)).await.expect("accepted");
    mine.commit(Change::Freeze).await.expect("accepted");

    assert_eq!(
        available(&runtime, Focus::all()).await.value,
        Reversals { undoable: false, redoable: false },
        "a change that declared no inverse is a barrier, not a candidate behind it"
    );
    assert!(
        matches!(mine.undo(alice(), Focus::all()).await, Err(Error::NothingToUndo)),
        "the read and the verb cannot disagree about the barrier"
    );
}

#[tokio::test]
async fn availability_per_principal() {
    let runtime = runtime().await;
    let mine = runtime.state.as_principal(alice());
    let theirs = runtime.state.as_principal(bob());

    mine.commit(deposit_to(VAULT, 7)).await.expect("accepted");
    theirs.commit(deposit(40)).await.expect("accepted");

    assert!(
        available(&runtime, Focus::all()).await.value.undoable,
        "bob's newer entry neither lights alice's button nor stands in its way"
    );
    assert!(!available(&runtime, only(support::HOUSE)).await.value.undoable, "alice's own entry is outside the scope being asked about");
}

pub(super) async fn history(runtime: &harmos_runtime::Runtime<support::Bank>) -> Vec<Position> {
    runtime.state.query::<Change>(Position::ORIGIN).await.map(|entry| entry.expect("the tail holds the whole history").position).collect()
}

pub(super) async fn available(runtime: &harmos_runtime::Runtime<support::Bank>, scope: Focus<support::Bank>) -> At<Reversals> {
    runtime.state.reversals(alice(), scope).await.expect("the writer answers")
}

fn only(account: support::Account) -> Focus<support::Bank> {
    Focus::matching(move |change: &Change| support::account(change) == Some(account))
}

/* === Tests === */
