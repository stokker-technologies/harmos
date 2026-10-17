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
use support::{Bank, Change, Ledger, booted, deposit, ledger, scratch};
// Local Imports
use super::support;
use harmos::{Error, state::Origin, state::Position, storage::Source};
use schematic_storage::Jsonl;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("storage_ac005_jsonl", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let copy = scratch("retention-jsonl").join("journal.jsonl");
                    let runtime = booted(Ledger::default(), |builder| {
                        builder.recover(Jsonl::<Bank>::open(&copy).expect("a fresh copy"))
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

                    let before = lines(&copy);
                    let mut source = Jsonl::<Bank>::open(&copy).expect("the copy is there");
                    Source::prune(&mut source, first.position).await;
                    let after = lines(&copy);

                    assert_eq!(&after[1..], &before[2..]);
                    let header: serde_json::Value =
                        serde_json::from_slice(&after[0]).expect("the schema header remains JSON");
                    assert_eq!(header["origin"], serde_json::json!(first.position.0));

                    let recovered = booted(origin, |builder| {
                        builder.recover(Jsonl::<Bank>::open(&copy).expect("the pruned copy opens"))
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
                });
        });
    }
}

fn lines(path: &std::path::Path) -> Vec<Vec<u8>> {
    std::fs::read(path)
        .expect("the copy is readable")
        .split_inclusive(|byte| *byte == b'\n')
        .map(<[u8]>::to_vec)
        .collect()
}

/* === Tests === */
