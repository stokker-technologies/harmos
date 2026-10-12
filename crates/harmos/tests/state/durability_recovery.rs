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
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;
// Third Party Imports
use serde::{Deserialize, Serialize};
use support::{Bank, Ledger, Noted, alice, booted, deposit, ledger, opened, scratch, withdraw};
// Local Imports
use super::support;
use harmos::state::Position;
use jsonl_adapter::{Jsonl, Snapshot};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Deserialize, Serialize)]
struct Saved {
    at: Position,
    state: Ledger,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("storage_ac001_recovered", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("recovered-boot");
                    let copy = directory.join("journal.jsonl");

                    let live = booted(Ledger::default(), |builder| {
                        builder.recover(Jsonl::<Bank>::open(&copy).expect("a fresh copy"))
                    })
                    .await;
                    let handle = live.state.as_principal(alice());
                    handle.commit(deposit(40)).await.expect("accepted");
                    handle.commit(withdraw(15)).await.expect("accepted");
                    live.records
                        .publish(Noted("audited".to_owned()))
                        .await
                        .expect("facts cannot be refused");
                    handle.commit(deposit(2)).await.expect("accepted");
                    let expected = ledger(&live).await;
                    let applied = live.state.applied();

                    // A derived handle is a handle: the writer stops when the last one goes.
                    drop(handle);

                    assert_eq!(
                        live.stop().await.unwrap(),
                        applied,
                        "shutdown drained the whole tail"
                    );

                    let recovered = booted(Ledger::default(), |builder| {
                        builder.recover(Jsonl::<Bank>::open(&copy).expect("the copy is there"))
                    })
                    .await;

                    assert_eq!(ledger(&recovered).await, expected);
                    assert_eq!(recovered.state.applied(), applied);
                    assert_eq!(expected.balance, 27);
                    assert_eq!(
                        expected.applications, 3,
                        "the fact took a position and applied nothing"
                    );
                });
        });
    }
    {
        scenario_runtime::check("unsynced_tail_thing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("risk-window");
                    let copy = directory.join("journal.jsonl");

                    // Three entries to a batch, so the fourth sits in the adapter's own risk
                    // window when the process dies.
                    let live = booted(Ledger::default(), |builder| {
                        builder.recover(
                            Jsonl::<Bank>::open(&copy)
                                .expect("a fresh copy")
                                .syncing_every(3),
                        )
                    })
                    .await;
                    for amount in [10, 20, 30, 40] {
                        live.state.commit(deposit(amount)).await.expect("accepted");
                    }
                    live.state.wait_until_stored(Position(3)).await;

                    assert_eq!(live.state.applied(), Position(4));
                    assert_eq!(
                        live.state.stored(),
                        Position(3),
                        "the fourth entry is inside the window, and nothing left the process on it"
                    );

                    // The crash: the process dies with its unsynced lines still in memory.
                    // Forgotten rather than stopped, because a stop would flush the held
                    // batch — a crash is exactly the path where nobody gets to.
                    std::mem::forget(live);

                    let recovered = booted(Ledger::default(), |builder| {
                        builder.recover(Jsonl::<Bank>::open(&copy).expect("the copy is there"))
                    })
                    .await;

                    assert_eq!(
                        ledger(&recovered).await.balance,
                        60,
                        "everything synced survived, and only the window was lost"
                    );
                    assert_eq!(recovered.state.applied(), Position(3));
                });
        });
    }
    {
        scenario_runtime::check("redelivery_forwards_stored", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let directory = scratch("byte-forward");
    let copy = directory.join("journal.jsonl");
    let mirror = directory.join("mirror.jsonl");

    // One entry as a version-1 build stored it, written valid but
    // non-canonical — a member order and spacing no serializer of this build
    // produces — so any re-encoding, of the payload or of the frame, fails
    // the byte comparison below.
    let historical = r#"{"kind":"Transaction","position":9,"id":"ledger.bonus","version":1,"metadata":{"principal":null,"correlation":9,"causation":null,"request":null,"link":null,"at_us":0},"payload":{ "amount":3 }}"#;
    // And one record this build has never heard of: it is skipped by the
    // fold, yet its line must still reach a durable copy whole.
    let unreadable = r#"{"position":10,"kind":"Record","id":"ledger.audited","version":1,"metadata":{"principal":null,"correlation":10,"causation":null,"request":null,"link":null,"at_us":0},"payload":{"note":"kept"}}"#;

    seeded(&copy).await;
    append(&copy, historical);
    append(&copy, unreadable);

    // A fresh mirror lags behind everything, so boot redelivers it the whole
    // recovered history.
    let recovered = booted(Ledger::default(), |builder| {
        builder
            .recover(Jsonl::<Bank>::open(&copy).expect("the copy is there"))
            .attach(Jsonl::<Bank>::open(&mirror).expect("a fresh mirror"))
    })
    .await;

    assert_eq!(
        ledger(&recovered).await.balance,
        16,
        "the version-1 bonus still applies through its own version"
    );
    assert_eq!(
        recovered.state.applied(),
        Position(10),
        "the unknown record was skipped, and its position was taken all the same"
    );
    recovered.stop().await.unwrap();

    let original = std::fs::read_to_string(&copy).expect("the copy is there");
    let written = std::fs::read_to_string(&mirror).expect("the mirror is there");

    assert_eq!(
        written, original,
        "a lagging durable copy is redelivered history's own bytes, so the \
         mirror equals its source line for line"
    );
    assert!(
        written.contains(historical),
        "the historical entry arrived verbatim, spacing and member order intact"
    );
    assert!(
        written.contains(unreadable),
        "a record this build cannot decode is still forwarded, not silently dropped"
    );

});
        });
    }
    {
        scenario_runtime::check("undecodable_history_beyond", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let directory = scratch("byte-lane-uncapped");
    let copy = directory.join("journal.jsonl");
    let mirror = directory.join("mirror.jsonl");

    // More lines than the redelivery window holds of entries, and not one of
    // them decodable by this build: the byte lane is the only way any of them
    // travels, so a window on that lane would silently amputate the copy.
    let mut file = String::from("{\"schema\":2,\"generation\":0,\"origin\":0}\n");
    for position in 1..=1030u64 {
        file.push_str(&format!(
            r#"{{"position":{position},"kind":"Record","id":"ledger.audited","version":1,"metadata":{{"principal":null,"correlation":{position},"causation":null,"request":null,"link":null,"at_us":0}},"payload":{{"note":"kept"}}}}"#
        ));
        file.push('\n');
    }
    std::fs::write(&copy, &file).expect("the copy is writable");

    let recovered = booted(Ledger::default(), |builder| {
        builder
            .recover(Jsonl::<Bank>::open(&copy).expect("the copy is there"))
            .attach(Jsonl::<Bank>::open(&mirror).expect("a fresh mirror"))
    })
    .await;

    assert_eq!(
        recovered.state.applied(),
        Position(1030),
        "every skipped record's position was taken all the same"
    );
    recovered.stop().await.unwrap();

    let written = std::fs::read_to_string(&mirror).expect("the mirror is there");

    assert_eq!(
        written,
        file.replacen("\"schema\":2", "\"schema\":3", 1),
        "a history of records only a later build can read reaches a fresh \
         mirror whole and byte-identical, with no window shortening it"
    );

});
        });
    }
    {
        scenario_runtime::check("ac002_clean_stop", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("stop-flushes");
                    let copy = directory.join("journal.jsonl");

                    // Three entries to a batch, so both commits sit in the adapter's own risk
                    // window when the stop begins.
                    let live = booted(Ledger::default(), |builder| {
                        builder.recover(
                            Jsonl::<Bank>::open(&copy)
                                .expect("a fresh copy")
                                .syncing_every(3),
                        )
                    })
                    .await;
                    for amount in [10, 20] {
                        live.state.commit(deposit(amount)).await.expect("accepted");
                    }
                    let settled = live.stop().await.unwrap();

                    // The settled frontier is read back after the flush; the drain alone had
                    // synced nothing under a three-entry batch.
                    assert_eq!(
                        settled,
                        Position(2),
                        "stop reports what the flush made durable, not what the drain had synced"
                    );

                    let recovered = booted(Ledger::default(), |builder| {
                        builder.recover(Jsonl::<Bank>::open(&copy).expect("the copy is there"))
                    })
                    .await;

                    assert_eq!(
                        ledger(&recovered).await.balance,
                        30,
                        "the held batch reached disk on stop rather than dying with the process"
                    );
                    assert_eq!(recovered.state.applied(), Position(2));
                });
        });
    }
    {
        scenario_runtime::check("stored_resolves_sync", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("gated-on-stored");
                    let copy = directory.join("journal.jsonl");
                    let runtime = booted(Ledger::default(), |builder| {
                        builder.recover(
                            Jsonl::<Bank>::open(&copy)
                                .expect("a fresh copy")
                                .syncing_every(2),
                        )
                    })
                    .await;

                    let first = runtime.state.commit(deposit(5)).await.expect("accepted");
                    let waiting = tokio::time::timeout(
                        Duration::from_millis(100),
                        runtime.state.wait_until_stored(first.position),
                    )
                    .await;

                    assert!(
                        waiting.is_err(),
                        "the entry is applied but not yet acknowledged by the copy"
                    );

                    runtime.state.commit(deposit(6)).await.expect("accepted");
                    tokio::time::timeout(
                        Duration::from_millis(500),
                        runtime.state.wait_until_stored(Position(2)),
                    )
                    .await
                    .expect("the batch was written and synced");

                    assert_eq!(runtime.state.stored(), runtime.state.applied());
                });
        });
    }
    {
        scenario_runtime::check("change_refuses_boot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let directory = scratch("unknown-definitions");
    let copy = directory.join("journal.jsonl");
    let known = directory.join("known.jsonl");

    seeded(&copy).await;
    std::fs::copy(&copy, &known).expect("the copy is there");
    append(
        &known,
        r#"{"position":9,"kind":"Record","id":"ledger.audited","version":1,"metadata":{"principal":null,"correlation":9,"causation":null,"request":null,"link":null,"at_us":0},"payload":{}}"#,
    );
    let skipping = booted(Ledger::default(), |builder| {
        builder.recover(Jsonl::<Bank>::open(&known).expect("the copy is there"))
    })
    .await;

    assert_eq!(
        ledger(&skipping).await.balance,
        10,
        "a fact this build never heard of mutates nothing, so it is skipped"
    );
    assert_eq!(skipping.state.applied(), Position(9));

    append(
        &copy,
        r#"{"position":9,"kind":"Transaction","id":"ledger.transfer","version":1,"metadata":{"principal":null,"correlation":9,"causation":null,"request":null,"link":null,"at_us":0},"payload":{"amount":3}}"#,
    );
    let booting = opened(Ledger::default(), |builder| {
        builder.recover(Jsonl::<Bank>::open(&copy).expect("the copy is there"))
    })
    .await;
    let Err(refused) = booting else {
        panic!("a change this build cannot honor ends the boot");
    };

    assert!(
        refused.to_string().contains("ledger.transfer"),
        "the refusal names what it could not honor: {refused}"
    );

});
        });
    }
    {
        scenario_runtime::check("version_refuses_boot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let directory = scratch("undeclared-version");
    let copy = directory.join("journal.jsonl");

    seeded(&copy).await;
    append(
        &copy,
        r#"{"position":9,"kind":"Transaction","id":"ledger.deposit","version":9,"metadata":{"principal":null,"correlation":9,"causation":null,"request":null,"link":null,"at_us":0},"payload":{"amount":3}}"#,
    );

    let booting = opened(Ledger::default(), |builder| {
        builder.recover(Jsonl::<Bank>::open(&copy).expect("the copy is there"))
    })
    .await;
    let Err(refused) = booting else {
        panic!("version 9 was never declared");
    };

    assert!(
        refused.to_string().contains("version 9"),
        "the refusal names the version it could not honor: {refused}"
    );

});
        });
    }
    {
        scenario_runtime::check("tail_restores_state", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let directory = scratch("snapshot-plus-tail");
                    let copy = directory.join("journal.jsonl");
                    let snapshot = directory.join("state.json");

                    let live = booted(Ledger::default(), |builder| {
                        builder
                            .recover(Jsonl::<Bank>::open(&copy).expect("a fresh copy"))
                            .snapshot(Snapshot::<Bank>::folding(
                                Ledger::default(),
                                Position::ORIGIN,
                                {
                                    let snapshot = snapshot.clone();
                                    move |state, at| {
                                        let file = File::create(&snapshot)?;

                                        serde_json::to_writer(
                                            file,
                                            &Saved {
                                                at,
                                                state: state.clone(),
                                            },
                                        )
                                        .map_err(std::io::Error::other)
                                    }
                                },
                            ))
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
                    let saved: Saved = serde_json::from_reader(
                        File::open(&snapshot).expect("a snapshot was taken"),
                    )
                    .expect("the application's own codec reads it");
                    let recovered = booted(saved.state, |builder| {
                        builder.recover(
                            Jsonl::<Bank>::open(&copy)
                                .expect("the copy is there")
                                .after(saved.at),
                        )
                    })
                    .await;

                    assert_eq!(
                        saved.at, applied,
                        "the snapshot names the position it holds"
                    );
                    assert_eq!(ledger(&recovered).await, expected);
                    assert_eq!(
                        recovered.state.applied(),
                        applied,
                        "an empty tail above the horizon still stands at the snapshot's position"
                    );
                });
        });
    }
}

async fn seeded(copy: &Path) {
    let runtime = booted(Ledger::default(), |builder| {
        builder.recover(Jsonl::<Bank>::open(copy).expect("a fresh copy"))
    })
    .await;

    for amount in [1, 2, 3, 4] {
        runtime
            .state
            .commit(deposit(amount * 2))
            .await
            .expect("accepted");
        runtime
            .state
            .commit(withdraw(amount))
            .await
            .expect("accepted");
    }
    runtime.stop().await.unwrap();
}

fn append(copy: &Path, line: &str) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(copy)
        .expect("the copy is there");

    writeln!(file, "{line}").expect("the copy is writable");
}

/* === Tests === */
