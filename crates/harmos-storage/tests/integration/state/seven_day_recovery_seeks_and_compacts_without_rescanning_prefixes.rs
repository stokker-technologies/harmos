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
use harmos_storage::state::{RecoveryEntry, RecoverySnapshot, StorageStateSettings};
use harmos_storage::{Error, Identifier, ReadLimits, RecoveryPosition, StorageCursor, StoreHistory};
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
fn seven_day_recovery_seeks_and_compacts_without_rescanning_prefixes() {
    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory {
        store: Identifier::new("journal.test").unwrap(),
        epoch: Identifier::new("one").unwrap(),
    };
    let mut journal = open(root.path(), &history);
    let per_day = 24 * 60;
    for day in 0..7 {
        let entries = (0..per_day)
            .map(|minute| {
                let sequence = (day * per_day + minute + 1) as u64;
                RecoveryEntry {
                    position: RecoveryPosition { history: history.clone(), sequence },
                    required_data: StorageCursor { history: history.clone(), sequence: 0 },
                    payload: sequence.to_le_bytes().to_vec(),
                }
            })
            .collect::<Vec<_>>();
        journal.append(&entries).unwrap();
    }
    let through = journal.position();
    drop(journal);
    let mut journal = open(root.path(), &history);
    assert_eq!(journal.offset_bytes().unwrap(), through.sequence * 8);
    let limits = ReadLimits {
        rows: NonZeroUsize::new(31).unwrap(),
        bytes: NonZeroUsize::new(16 * 1024).unwrap(),
    };
    let mut after = RecoveryPosition { history: history.clone(), sequence: 0 };
    let mut pages = 0;
    while after.sequence < through.sequence {
        let page = journal.page(&after, &through, limits).unwrap();
        for (index, entry) in page.entries.iter().enumerate() {
            assert_eq!(entry.position.sequence, after.sequence + index as u64 + 1);
            assert_eq!(entry.payload, entry.position.sequence.to_le_bytes());
        }
        after = page.entries.last().unwrap().position.clone();
        pages += 1;
    }
    assert!(journal.decoded() <= through.sequence as usize + pages);
    // Arbitrary Retry Pages Also Seek Directly Rather Than Replaying History.
    journal.reset_decoded();
    let retry_after = RecoveryPosition {
        history: history.clone(),
        sequence: through.sequence - 3,
    };
    assert_eq!(journal.page(&retry_after, &through, limits).unwrap().entries.len(), 3);
    assert_eq!(journal.decoded(), 3);
    let old_journal = std::fs::read(root.path().join("state.jsonl")).unwrap();
    let checkpoint = RecoveryPosition {
        history: history.clone(),
        sequence: through.sequence - 10,
    };
    let snapshot = RecoverySnapshot {
        through: checkpoint.clone(),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: harmos_storage::state::SnapshotPayload::from_bytes(
            &vec![7],
            std::num::NonZeroUsize::new(65536).unwrap(),
            harmos_domain::state::Position::ORIGIN,
        )
        .unwrap(),
    };
    journal.checkpoint(&snapshot).unwrap();
    assert_eq!(journal.offset_bytes().unwrap(), 10 * 8);
    assert_eq!(journal.page(&checkpoint, &through, limits).unwrap().entries.len(), 10);
    drop(journal);
    // A Crash After Snapshot Publication But Before Journal Replacement.
    std::fs::write(root.path().join("state.jsonl"), old_journal).unwrap();
    let mut journal = open(root.path(), &history);
    assert_eq!(journal.snapshot(4096).unwrap(), Some(snapshot));
    assert_eq!(journal.page(&checkpoint, &through, limits).unwrap().entries.len(), 10);
    assert_eq!(journal.decoded(), 10);
    assert!(matches!(journal.page(&retry_after, &checkpoint, limits), Err(Error::InvalidInput { .. })));
}
