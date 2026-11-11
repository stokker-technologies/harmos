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

// Local Imports
use super::Model;
use harmos::storage::Source;
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

#[test]
fn contract_scenario() {
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("journal.jsonl");
                std::fs::write(&path, "{\"schema\":1,\"generation\":0,\"origin\":0}\n").unwrap();
                let mut log = Jsonl::<Model>::open(&path).unwrap();
                std::fs::write(&path, vec![b' '; 8192]).unwrap();
                assert!(Jsonl::<Model>::open(&path).is_err());
                assert!(log.load().await.is_err());
            });
        }
    }
}

#[test]
fn incompatible_records_refuse_without_rewriting_root() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        use super::support::{Bank, Ledger, Noted, booted};
        use harmos::Identified;

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("journal.jsonl");
        let runtime = booted(Ledger::default(), |setup| {
            setup.settings().recover(Jsonl::<Bank>::open(&path).unwrap());
        })
        .await;
        runtime.records.publish(Noted("saved".into())).await.unwrap();
        runtime.shutdown().await.unwrap();
        let original = std::fs::read_to_string(&path).unwrap();
        let revision = Noted::definition().1;
        let changed = revision.wrapping_add(1).max(1);
        for incompatible in [
            original.replace("ledger.noted", "ledger.removed"),
            original.replace(&format!("\"version\":{revision}"), &format!("\"version\":{changed}")),
        ] {
            std::fs::write(&path, &incompatible).unwrap();
            let mut journal = Jsonl::<Bank>::open(&path).unwrap();
            assert!(journal.load().await.is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), incompatible);
        }
        std::fs::write(&path, &original).unwrap();
        let mut journal = Jsonl::<Bank>::open(&path).unwrap();
        assert_eq!(journal.load().await.unwrap().entries.len(), 1);
    });
}

/* === Tests === */
