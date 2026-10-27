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
use std::{fs::OpenOptions, io::Write, num::NonZeroUsize};
// Third Party Imports
// Local Imports
use harmos_runtime::storage::state::{RecoveryEntry, RecoverySnapshot, StateBackend, StorageStateSettings, file::FileBackend};
use harmos_runtime::storage::{ReadLimits, RecoveryPosition, StorageCursor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn checkpoint_survives_reopen() {
    let memory = super::super::memory::read().await;
    let root = tempfile::tempdir().unwrap();
    let history = crate::storage::history();
    let settings = crate::storage::state_settings();
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    let at = |sequence| RecoveryPosition { history: history.clone(), sequence };
    let required_data = StorageCursor { history: history.clone(), sequence: 0 };
    assert_eq!(backend.open().await.unwrap(), at(0));
    let entries = (1..=3)
        .map(|sequence| RecoveryEntry {
            position: at(sequence),
            required_data: required_data.clone(),
            payload: vec![sequence as u8],
        })
        .collect::<Vec<_>>();
    backend.write_entries(harmos_storage::StorageBatch::new(entries.clone())).await.unwrap();
    assert_eq!(backend.flush(&at(3)).await.unwrap(), at(3));
    backend.write_entries(harmos_storage::StorageBatch::new(entries.clone())).await.unwrap();
    let snapshot = RecoverySnapshot {
        through: at(2),
        required_data,
        payload: b"checkpoint".to_vec(),
    };
    backend.write_snapshot(harmos_storage::StorageBatch::new(vec![snapshot.clone()])).await.unwrap();
    backend.close().await.unwrap();

    let mut file = OpenOptions::new().append(true).open(root.path().join("state.jsonl")).unwrap();
    file.write_all(b"{\"value\":").unwrap();
    file.sync_all().unwrap();
    drop(file);

    let reopened = FileBackend::new(root.path().to_owned(), history, settings);
    assert_eq!(reopened.open().await.unwrap().sequence, 3);
    assert_eq!(
        reopened.read_snapshot(std::num::NonZeroUsize::new(1024 * 1024).unwrap(), memory.clone()).await.unwrap(),
        Some(snapshot.clone())
    );
    let through = RecoveryPosition {
        history: snapshot.through.history.clone(),
        sequence: 3,
    };
    let page = reopened
        .read_entries(
            &snapshot.through,
            &through,
            ReadLimits {
                rows: std::num::NonZeroUsize::new(64).unwrap(),
                bytes: std::num::NonZeroUsize::new(64 * 1024).unwrap(),
            },
            memory.clone(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries, entries[2..]);
    assert_eq!(page.next, None);
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn corruption_refused_preserved() {
    let root = tempfile::tempdir().unwrap();
    let history = crate::storage::history();
    let settings = crate::storage::state_settings();
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    backend.open().await.unwrap();
    let position = RecoveryPosition { history: history.clone(), sequence: 1 };
    let entry = RecoveryEntry {
        position: position.clone(),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: b"saved state".to_vec(),
    };
    backend.write_entries(harmos_storage::StorageBatch::new(vec![entry.clone()])).await.unwrap();
    backend.flush(&position).await.unwrap();
    backend.close().await.unwrap();

    // A Complete Entry With A Damaged Payload Must Not Be Treated As An Interrupted Append.
    let path = root.path().join("state.jsonl");
    let mut corrupt = std::fs::read(&path).unwrap();
    let encoded = b"c2F2ZWQgc3RhdGU";
    let at = corrupt.windows(encoded.len()).position(|window| window == encoded).unwrap();
    corrupt[at] = b'd';
    std::fs::write(&path, &corrupt).unwrap();
    let reopened = FileBackend::new(root.path().to_owned(), history, settings);
    assert!(reopened.open().await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    assert!(reopened.open().await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
}

#[tokio::test]
async fn state_root_exclusive() {
    let memory = super::super::memory::read().await;
    let root = tempfile::tempdir().unwrap();
    let history = crate::storage::history();
    let settings = crate::storage::state_settings();
    let first = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    let second = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    first.open().await.unwrap();
    assert!(second.open().await.is_err());
    let position = RecoveryPosition { history: history.clone(), sequence: 1 };
    let entry = RecoveryEntry {
        position: position.clone(),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: b"exclusive writer".to_vec(),
    };
    first.write_entries(harmos_storage::StorageBatch::new(vec![entry.clone()])).await.unwrap();
    first.flush(&position).await.unwrap();
    first.close().await.unwrap();

    // A Refused Owner Can Acquire The Root After The Original Owner Closes.
    assert_eq!(second.open().await.unwrap(), position);
    let after = RecoveryPosition { history, sequence: 0 };
    let page = second
        .read_entries(
            &after,
            &position,
            ReadLimits {
                rows: std::num::NonZeroUsize::new(64).unwrap(),
                bytes: std::num::NonZeroUsize::new(64 * 1024).unwrap(),
            },
            memory.clone(),
        )
        .await
        .unwrap();
    assert_eq!(page.entries, vec![entry]);
    assert_eq!(page.next, None);
    assert!(first.open().await.is_err());
    second.close().await.unwrap();
    assert_eq!(first.open().await.unwrap(), position);
    first.close().await.unwrap();
}

#[tokio::test]
async fn limits_bound_records() {
    let memory = super::super::memory::read().await;
    let history = crate::storage::history();
    let at = |sequence| RecoveryPosition { history: history.clone(), sequence };
    let entry = |sequence, bytes: usize| RecoveryEntry {
        position: at(sequence),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: vec![7; bytes],
    };
    let snapshot = |bytes: usize| RecoverySnapshot {
        through: at(1),
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        payload: vec![9; bytes],
    };

    // Measure The Encoded Records Written For A Payload Of 3k Bytes.
    let measured = tempfile::tempdir().unwrap();
    let journal = measured.path().join("state.jsonl");
    let backend = FileBackend::new(measured.path().to_owned(), history.clone(), crate::storage::state_settings());
    backend.open().await.unwrap();
    let header = std::fs::metadata(&journal).unwrap().len();
    backend.write_entries(harmos_storage::StorageBatch::new(vec![entry(1, 3000)])).await.unwrap();
    // Measure Before The Checkpoint Compacts The Journal Back To Its Header.
    let entry_bytes = std::fs::metadata(&journal).unwrap().len() - header;
    backend.write_snapshot(harmos_storage::StorageBatch::new(vec![snapshot(3000)])).await.unwrap();
    backend.close().await.unwrap();
    let checkpoint_bytes = std::fs::metadata(measured.path().join("state.json")).unwrap().len();
    // Base64 Text Plus Envelope, Far Below The Former Decimal Number Arrays.
    assert!(entry_bytes < 4500 && checkpoint_bytes < 4500);

    // A Payload At The Limit Is Accepted; One Byte More Is Refused And Appends Nothing.
    let root = tempfile::tempdir().unwrap();
    let settings = StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(entry_bytes as usize).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(checkpoint_bytes as usize).unwrap(),
    };
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    backend.open().await.unwrap();
    backend.write_entries(harmos_storage::StorageBatch::new(vec![entry(1, 3000)])).await.unwrap();
    assert!(backend.write_entries(harmos_storage::StorageBatch::new(vec![entry(2, 3001)])).await.is_err());
    assert!(backend.write_snapshot(harmos_storage::StorageBatch::new(vec![snapshot(3001)])).await.is_err());
    backend.write_snapshot(harmos_storage::StorageBatch::new(vec![snapshot(3000)])).await.unwrap();
    backend.close().await.unwrap();

    let reopened = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    assert_eq!(reopened.open().await.unwrap(), at(1));
    assert_eq!(
        reopened.read_snapshot(NonZeroUsize::new(checkpoint_bytes as usize).unwrap(), memory.clone()).await.unwrap(),
        Some(snapshot(3000))
    );
    // A Caller Bound Covers The Snapshot Value, Not The Integrity Envelope.
    let value_bytes = serde_json::to_vec(&snapshot(3000)).unwrap().len();
    assert_eq!(reopened.read_snapshot(NonZeroUsize::new(value_bytes).unwrap(), memory.clone()).await.unwrap(), Some(snapshot(3000)));
    reopened.close().await.unwrap();
}
