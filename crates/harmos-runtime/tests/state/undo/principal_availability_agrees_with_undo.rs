/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test]
async fn principal_availability_agrees_with_undo() {
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

    // Bob's newer entry neither blocks alice's undo nor is chosen by it.
    mine.undo(alice(), Focus::all()).await.expect("alice's own entry is still the candidate");

    assert_eq!(support::balance(&runtime, VAULT).await, 0);
    assert_eq!(support::balance(&runtime, support::HOUSE).await, 40);
    assert!(
        matches!(mine.undo(alice(), Focus::all()).await, Err(Error::NothingToUndo)),
        "alice cannot reach into bob's history for a second undo"
    );
}
