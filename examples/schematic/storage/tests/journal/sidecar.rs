/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

use super::support;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs::File;
use std::io;
use std::path::Path;
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos::state::Position;
use schematic_storage::Jsonl;
use support::{Bank, Ledger, booted, deposit, ledger, scratch};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Deserialize, Serialize)]
struct Artifact {
    generation: u64,
    at: Position,
    state: Ledger,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let directory = scratch("sidecar-restores");
                let (artifact, sidecar) = (directory.join("book.json"), directory.join("book.jsonl"));

                write(&artifact, 1, Position::ORIGIN, &Ledger::default());
                let opened = booted(Ledger::default(), |setup| {
                    setup.settings().recover(Jsonl::<Bank>::sidecar(&sidecar, 1, Position::ORIGIN).expect("a fresh sidecar"));
                })
                .await;
                opened.state.commit(deposit(30)).await.expect("accepted");
                opened.state.commit(deposit(12)).await.expect("accepted");
                let unsaved = ledger(&opened).await;

                // The crash: nothing was saved, so the artifact is still the empty book.
                opened.shutdown().await.unwrap();

                let saved = read(&artifact);
                let recovered = booted(saved.state, |setup| {
                    setup.settings().recover(Jsonl::<Bank>::sidecar(&sidecar, saved.generation, saved.at).expect("the sidecar is there"));
                })
                .await;

                assert_eq!(ledger(&recovered).await, unsaved, "no work was lost");
                assert_eq!(recovered.state.applied(), Position(2));
            });
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let directory = scratch("sidecar-mismatch");
                let (artifact, sidecar) = (directory.join("book.json"), directory.join("book.jsonl"));

                write(&artifact, 1, Position::ORIGIN, &Ledger::default());
                let opened = booted(Ledger::default(), |setup| {
                    setup.settings().recover(Jsonl::<Bank>::sidecar(&sidecar, 1, Position::ORIGIN).expect("a fresh sidecar"));
                })
                .await;
                opened.state.commit(deposit(30)).await.expect("accepted");
                opened.shutdown().await.unwrap();

                // The artifact moved on without this sidecar — saved elsewhere, or by a
                // half-landed save whose reset never ran. Either way it belongs to
                // generation 1 and the book in hand is generation 2.
                let elsewhere = Ledger {
                    balance: 500,
                    applications: 1,
                    payouts: Vec::new(),
                };
                write(&artifact, 2, Position(9), &elsewhere);

                let saved = read(&artifact);
                let recovered = booted(saved.state.clone(), |setup| {
                    setup.settings().recover(Jsonl::<Bank>::sidecar(&sidecar, saved.generation, saved.at).expect("the sidecar is there"));
                })
                .await;

                assert_eq!(ledger(&recovered).await, elsewhere, "the sidecar folded from another save is not applied to this artifact");
                assert_eq!(recovered.state.applied(), Position(9), "the runtime stands where the artifact says it does");
            });
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let directory = scratch("sidecar-save");
                let (artifact, sidecar) = (directory.join("book.json"), directory.join("book.jsonl"));

                write(&artifact, 1, Position::ORIGIN, &Ledger::default());
                let copy = Jsonl::<Bank>::sidecar(&sidecar, 1, Position::ORIGIN).expect("a fresh sidecar");
                let saves = copy.sidecars();
                let opened = booted(Ledger::default(), |setup| {
                    setup.settings().recover(copy);
                })
                .await;

                opened.state.commit(deposit(30)).await.expect("accepted");
                opened.state.wait_until_stored(Position(1)).await;
                let exported = ledger(&opened).await;

                // A refused export leaves both halves exactly as they were.
                let refused = saves.save(|_generation, _at| Err(io::Error::other("the disk is full")));

                assert!(refused.is_err());
                assert_eq!(read(&artifact).generation, 1);
                assert_eq!(saves.generation(), 1, "the sidecar was not reset");

                let generation = saves
                    .save(|generation, at| {
                        write(&artifact, generation, at, &exported);

                        Ok(())
                    })
                    .expect("the export landed");

                assert_eq!(generation, 2);
                assert_eq!(read(&artifact).generation, 2);
                assert_eq!(saves.generation(), 2, "both halves landed together");
                assert_eq!(lines(&sidecar), 1, "a reset sidecar is its header alone");

                opened.state.commit(deposit(4)).await.expect("accepted");
                opened.shutdown().await.unwrap();

                let saved = read(&artifact);
                let recovered = booted(saved.state, |setup| {
                    setup.settings().recover(Jsonl::<Bank>::sidecar(&sidecar, saved.generation, saved.at).expect("the sidecar is there"));
                })
                .await;

                assert_eq!(ledger(&recovered).await.balance, 34, "the artifact carries what was saved and the sidecar what came after");
                assert_eq!(recovered.state.applied(), Position(2));
            });
        }
    }
}

fn write(path: &Path, generation: u64, at: Position, state: &Ledger) {
    let file = File::create(path).expect("the directory is writable");

    serde_json::to_writer(file, &Artifact { generation, at, state: state.clone() }).expect("a ledger serializes");
}

fn read(path: &Path) -> Artifact {
    serde_json::from_reader(File::open(path).expect("the artifact is there")).expect("the application's own codec reads it")
}

fn lines(path: &Path) -> usize {
    std::fs::read_to_string(path).expect("the sidecar is there").lines().count()
}

/* === Tests === */
