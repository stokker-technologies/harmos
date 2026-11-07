/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State Journal Recovery And Indexing

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_storage::state::file::Journal;
use harmos_storage::state::{RecoveryEntry, StorageStateSettings};
use harmos_storage::{Error, Identifier, StorageCursor, StoreHistory};
use std::{fs::OpenOptions, num::NonZeroUsize, path::Path, sync::Arc};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

fn settings() -> StorageStateSettings {
    StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(4096).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap(),
    }
}

fn open(path: &Path, history: &StoreHistory) -> Journal {
    Journal::open(
        path,
        history.clone(),
        settings(),
        Arc::new(|path| OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path)),
        &mut || Ok(()),
    )
    .unwrap()
}

/* === Private Functions === */

/* === Tests === */

#[test]
fn index_open_cancellation_preserves_the_durable_journal() {
    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory {
        store: Identifier::new("cancel.test").unwrap(),
        epoch: Identifier::new("one").unwrap(),
    };
    let mut journal = open(root.path(), &history);
    let entries = (1..=8)
        .map(|sequence| RecoveryEntry {
            position: RecoveryPosition { history: history.clone(), sequence },
            required_data: StorageCursor { history: history.clone(), sequence: 0 },
            payload: vec![sequence as u8],
        })
        .collect::<Vec<_>>();
    journal.append(&entries).unwrap();
    drop(journal);
    let durable = std::fs::read(root.path().join("state.jsonl")).unwrap();
    let mut polls = 0;
    let cancelled = Journal::open(
        root.path(),
        history.clone(),
        settings(),
        Arc::new(|path| OpenOptions::new().read(true).write(true).open(path)),
        &mut || {
            polls += 1;
            if polls == 3 { Err(Error::Closing { domain: "test cancellation" }) } else { Ok(()) }
        },
    );
    assert!(matches!(cancelled, Err(Error::Closing { .. })));
    assert_eq!(std::fs::read(root.path().join("state.jsonl")).unwrap(), durable);
    assert_eq!(open(root.path(), &history).position().sequence, 8);
}
