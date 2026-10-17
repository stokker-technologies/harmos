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

// Third Party Imports
use support::{Change, Refusal, VAULT, alice, bob, deposit, deposit_to, ledger, runtime, withdraw};
// Local Imports
use crate::support;
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

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("journal_ac005_nothing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mine = runtime.state.as_principal(alice());
                    let theirs = runtime.state.as_principal(bob());

                    mine.commit(deposit(40)).await.expect("accepted");
                    theirs
                        .commit(withdraw(40))
                        .await
                        .expect("the world moves on");

                    let before = ledger(&runtime).await;
                    let frontier = runtime.state.applied();

                    let refused = mine.undo(alice(), Focus::all()).await;

                    assert!(matches!(
                        refused,
                        Err(Error::UndoConflict(Refusal::Overdrawn))
                    ));
                    assert_eq!(
                        runtime.state.applied(),
                        frontier,
                        "a conflicting undo appends nothing"
                    );
                    assert_eq!(
                        ledger(&runtime).await,
                        before,
                        "the inverse ran its own check before anything mutated"
                    );
                    assert_eq!(history(&runtime).await, vec![Position(1), Position(2)]);
                });
        });
    }
    {
        scenario_runtime::check("journal_ac005_undo", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mine = runtime.state.as_principal(alice());

                    let taken = mine.commit(deposit(40)).await.expect("accepted");
                    let answer = mine
                        .undo(alice(), Focus::all())
                        .await
                        .expect("alice has an entry to undo");

                    assert_eq!(answer.position, Position(2), "history got longer");
                    assert_eq!(support::balance(&runtime, support::HOUSE).await, 0);
                    assert_eq!(
                        history(&runtime).await,
                        vec![taken.position, answer.position]
                    );

                    let restored = mine
                        .redo(alice(), Focus::all())
                        .await
                        .expect("the undone entry is redoable");

                    assert_eq!(restored.position, Position(3));
                    assert_eq!(support::balance(&runtime, support::HOUSE).await, 40);
                });
        });
    }
    {
        scenario_runtime::check("undo_selects_within", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mine = runtime.state.as_principal(alice());
                    let theirs = runtime.state.as_principal(bob());

                    mine.commit(deposit_to(VAULT, 7)).await.expect("accepted");
                    theirs.commit(deposit(40)).await.expect("accepted");

                    // Bob's newer entry neither blocks alice's undo nor is chosen by it.
                    mine.undo(alice(), Focus::all())
                        .await
                        .expect("alice's own entry is still the candidate");

                    assert_eq!(support::balance(&runtime, VAULT).await, 0);
                    assert_eq!(support::balance(&runtime, support::HOUSE).await, 40);
                    assert!(
                        matches!(
                            mine.undo(alice(), Focus::all()).await,
                            Err(Error::NothingToUndo)
                        ),
                        "alice cannot reach into bob's history for a second undo"
                    );
                });
        });
    }
    {
        scenario_runtime::check("reversals_report_undo", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mine = runtime.state.as_principal(alice());

                    assert_eq!(
                        available(&runtime, Focus::all()).await.value,
                        Reversals {
                            undoable: false,
                            redoable: false,
                        },
                        "an empty history offers neither direction"
                    );

                    let taken = mine.commit(deposit(40)).await.expect("accepted");
                    let answered = available(&runtime, Focus::all()).await;

                    assert_eq!(
                        answered.value,
                        Reversals {
                            undoable: true,
                            redoable: false,
                        }
                    );
                    assert_eq!(
                        answered.position, taken.position,
                        "the answer says which point in history it reflects"
                    );

                    mine.undo(alice(), Focus::all()).await.expect("undone");
                    assert_eq!(
                        available(&runtime, Focus::all()).await.value,
                        Reversals {
                            undoable: false,
                            redoable: true,
                        },
                        "the entry moved from one side to the other"
                    );

                    mine.redo(alice(), Focus::all()).await.expect("restored");
                    assert_eq!(
                        available(&runtime, Focus::all()).await.value,
                        Reversals {
                            undoable: true,
                            redoable: false,
                        }
                    );
                });
        });
    }
    {
        scenario_runtime::check("change_never_available", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mine = runtime.state.as_principal(alice());

                    mine.commit(deposit(40)).await.expect("accepted");
                    mine.commit(Change::Freeze).await.expect("accepted");

                    assert_eq!(
                        available(&runtime, Focus::all()).await.value,
                        Reversals {
                            undoable: false,
                            redoable: false,
                        },
                        "a change that declared no inverse is a barrier, not a candidate behind it"
                    );
                    assert!(
                        matches!(
                            mine.undo(alice(), Focus::all()).await,
                            Err(Error::NothingToUndo)
                        ),
                        "the read and the verb cannot disagree about the barrier"
                    );
                });
        });
    }
    {
        scenario_runtime::check("availability_per_principal", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mine = runtime.state.as_principal(alice());
                    let theirs = runtime.state.as_principal(bob());

                    mine.commit(deposit_to(VAULT, 7)).await.expect("accepted");
                    theirs.commit(deposit(40)).await.expect("accepted");

                    assert!(
                        available(&runtime, Focus::all()).await.value.undoable,
                        "bob's newer entry neither lights alice's button nor stands in its way"
                    );
                    assert!(
                        !available(&runtime, only(support::HOUSE))
                            .await
                            .value
                            .undoable,
                        "alice's own entry is outside the scope being asked about"
                    );
                });
        });
    }
    {
        scenario_runtime::check("asking_what_nothing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mine = runtime.state.as_principal(alice());

                    mine.commit(deposit(40)).await.expect("accepted");
                    let frontier = runtime.state.applied();

                    available(&runtime, Focus::all()).await;
                    available(&runtime, Focus::all()).await;

                    assert_eq!(
                        runtime.state.applied(),
                        frontier,
                        "a question about undo is a read, not a commit"
                    );
                });
        });
    }
    {
        scenario_runtime::check("forward_commit_clears", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mine = runtime.state.as_principal(alice());

                    mine.commit(deposit(40)).await.expect("accepted");
                    mine.undo(alice(), Focus::all()).await.expect("undone");

                    mine.commit(deposit(7)).await.expect("the present diverges");

                    assert!(
                        matches!(
                            mine.redo(alice(), Focus::all()).await,
                            Err(Error::NothingToRedo)
                        ),
                        "a new forward commit truncates the redo branch"
                    );
                });
        });
    }
}

async fn history(runtime: &harmos_runtime::Runtime<support::Bank>) -> Vec<Position> {
    runtime
        .state
        .query::<Change>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("the tail holds the whole history").position)
        .collect()
}

async fn available(
    runtime: &harmos_runtime::Runtime<support::Bank>,
    scope: Focus<support::Bank>,
) -> At<Reversals> {
    runtime
        .state
        .reversals(alice(), scope)
        .await
        .expect("the writer answers")
}

fn only(account: support::Account) -> Focus<support::Bank> {
    Focus::matching(move |change: &Change| support::account(change) == Some(account))
}

/* === Tests === */
