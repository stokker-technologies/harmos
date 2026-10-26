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
use harmos::{state::Position, storage::JournalConsumer, storage::Source};
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
                std::fs::write(&path, "{\"schema\":3,\"generation\":0,\"origin\":0}\n").unwrap();
                let mut log = Jsonl::<Model>::open(&path).unwrap();
                std::fs::write(&path, vec![b' '; 8192]).unwrap();
                assert!(Jsonl::<Model>::open(&path).is_err());
                assert!(log.load().await.is_err());
            });
        }
    }
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("journal.jsonl");
    let original = "{\"schema\":2,\"generation\":0,\"origin\":0}\n";
    std::fs::write(&path, original).unwrap();
    let obstacle = path.with_extension("upgrade~");
    std::fs::create_dir(&obstacle).unwrap();
    let mut log = Jsonl::<Model>::open(&path).unwrap();
    let line = r#"{"group":1,"first":1,"frontier":1,"entries":[{"position":1,"kind":"Transaction","id":"atomic.add","version":1,"metadata":{"principal":null,"correlation":1,"causation":null,"request":null,"link":null,"at_us":0},"payload":{"amount":1}}]}"#;
    let shared = std::sync::Arc::new(line.as_bytes().to_vec());
    log.redelivered(&[(Position(1), std::sync::Arc::clone(&shared))]);
    assert_eq!(log.drain(vec![], Position(1)).await, Position::ORIGIN);
    assert_eq!(std::sync::Arc::strong_count(&shared), 2);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert_eq!(
        log.resume(),
        Position(1),
        "the pending entry remains accepted"
    );

    std::fs::remove_dir(&obstacle).unwrap();
    log.flush().await;
    assert_eq!(std::sync::Arc::strong_count(&shared), 1);
    assert_eq!(log.drain(vec![], Position(1)).await, Position(1));
    let recovery = Jsonl::<Model>::open(&path).unwrap().load().await.unwrap();
    assert_eq!(recovery.frontier, Position(1));
    assert_eq!(recovery.entries.len(), 1);
    let persisted = std::fs::read_to_string(&path).unwrap();
    let upgraded = "{\"schema\":3,\"generation\":0,\"origin\":0}\n";
    assert_eq!(persisted, format!("{upgraded}{line}\n"));
    log.flush().await;
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        persisted,
        "successful persistence clears pending entries and never repeats them"
    );

});
        }
    }
}

/* === Tests === */
