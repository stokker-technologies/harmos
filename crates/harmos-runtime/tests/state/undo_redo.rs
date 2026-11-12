/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use support::{alice, deposit, runtime};
// Local Imports
use crate::state::support;
use harmos_runtime::{Error, state::Focus, state::Position, state::Reversals};

/* === Private Functions === */

use super::undo::{available, history};

#[tokio::test]
async fn commits_undo_redo_and_new_history_update_availability() {
    // Start With Empty History
    let runtime = runtime().await;
    let mine = runtime.state.as_principal(alice());
    assert_eq!(available(&runtime, Focus::all()).await.value, Reversals { undoable: false, redoable: false });

    // Commit And Inspect Availability Without Appending
    let taken = mine.commit(deposit(40)).await.unwrap();
    let answered = available(&runtime, Focus::all()).await;
    assert_eq!(answered.value, Reversals { undoable: true, redoable: false });
    assert_eq!(answered.position, taken.position);
    available(&runtime, Focus::all()).await;
    assert_eq!(runtime.state.applied(), taken.position);

    // Undo Appends An Inverse And Enables Redo
    let undone = mine.undo(alice(), Focus::all()).await.unwrap();
    assert_eq!(undone.position, Position(2));
    assert_eq!(support::balance(&runtime, support::HOUSE).await, 0);
    assert_eq!(history(&runtime).await, vec![taken.position, undone.position]);
    assert_eq!(available(&runtime, Focus::all()).await.value, Reversals { undoable: false, redoable: true });

    // Redo Appends The Original Change
    let restored = mine.redo(alice(), Focus::all()).await.unwrap();
    assert_eq!(restored.position, Position(3));
    assert_eq!(support::balance(&runtime, support::HOUSE).await, 40);
    assert_eq!(available(&runtime, Focus::all()).await.value, Reversals { undoable: true, redoable: false });

    // A New Forward Commit Replaces The Redo Branch
    mine.undo(alice(), Focus::all()).await.unwrap();
    mine.commit(deposit(7)).await.unwrap();
    assert_eq!(available(&runtime, Focus::all()).await.value, Reversals { undoable: true, redoable: false });
    assert!(matches!(mine.redo(alice(), Focus::all()).await, Err(Error::NothingToRedo)));
    assert_eq!(support::balance(&runtime, support::HOUSE).await, 7);

    // Stop
    crate::support::app::shutdown(&runtime).await.unwrap();
}
