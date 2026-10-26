/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State Journal Failure And Reopen Integration Story

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    fs::{self, OpenOptions},
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

// Third Party Imports

// Local Imports
use harmos_storage::state::{RecoveryEntry, StateBackend, StorageStateSettings, file::FileBackend};
use harmos_storage::{Identifier, ReadLimits, RecoveryPosition, StorageCursor, StoreHistory};

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
async fn rollback_requires_reopen() {
    let memory = crate::manager::backend::read_memory().await;
    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory { store: Identifier::new("state.test").unwrap(), epoch: Identifier::new("epoch.one").unwrap() };
    let settings =
        StorageStateSettings { max_entry_bytes: NonZeroUsize::new(4096).unwrap(), max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap() };
    let initial = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    let start = initial.open().await.unwrap();
    assert_eq!(start.sequence, 0);
    initial.close().await.unwrap();
    let journal_path = root.path().join("state.jsonl");
    let acknowledged = fs::read(&journal_path).unwrap();

    // A Real Read-Only Handle Refuses Both Append And Rollback.
    let writable = Arc::new(AtomicBool::new(false));
    let access = Arc::clone(&writable);
    let backend = FileBackend::with_journal_opener(root.path().to_owned(), history.clone(), settings, move |path| {
        OpenOptions::new().read(true).write(access.load(Ordering::SeqCst)).open(path)
    });
    assert_eq!(backend.open().await.unwrap(), start);
    let entry = RecoveryEntry {
        position: RecoveryPosition { history: history.clone(), sequence: 1 },
        required_data: StorageCursor { history, sequence: 0 },
        payload: vec![1],
    };
    let entries = [entry];
    assert!(backend.write_entries(harmos_storage::StorageBatch::new(entries.to_vec())).await.is_err());
    assert_eq!(fs::read(&journal_path).unwrap(), acknowledged);

    // Restoring Access Cannot Clear An Unresolved Journal Failure.
    writable.store(true, Ordering::SeqCst);
    let poisoned = backend.open().await.unwrap_err();
    let diagnostic = poisoned.to_string();
    assert!(diagnostic.contains("requires close and reopen"), "{diagnostic}");
    assert!(backend.write_entries(harmos_storage::StorageBatch::new(entries.to_vec())).await.is_err());
    assert!(backend.flush(&start).await.is_err());
    assert!(backend.close().await.is_err());

    // Reopen Preserves The Acknowledged Prefix And Allows A Durable Retry.
    assert_eq!(backend.open().await.unwrap(), start);
    backend.write_entries(harmos_storage::StorageBatch::new(entries.to_vec())).await.unwrap();
    let through = backend.flush(&entries[0].position).await.unwrap();
    assert_eq!(through.sequence, 1);
    let limits = ReadLimits { rows: NonZeroUsize::new(1).unwrap(), bytes: NonZeroUsize::new(4096).unwrap() };
    let page = backend.read_entries(&start, &through, limits, memory.clone()).await.unwrap();
    assert_eq!(page.entries, entries);
    assert!(page.next.is_none());
    backend.close().await.unwrap();
}

#[tokio::test]
async fn snapshot_requires_singleton() {
    let memory = crate::manager::backend::read_memory().await;
    use harmos_storage::{StorageBatch, state::RecoverySnapshot};
    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory { store: Identifier::new("snapshot.test").unwrap(), epoch: Identifier::new("one").unwrap() };
    let settings =
        StorageStateSettings { max_entry_bytes: NonZeroUsize::new(4096).unwrap(), max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap() };
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    let through = backend.open().await.unwrap();
    let snapshot = RecoverySnapshot { required_data: StorageCursor { history, sequence: 0 }, through, payload: vec![7] };
    assert!(backend.write_snapshot(StorageBatch::new(Vec::new())).await.is_err());
    assert!(backend.write_snapshot(StorageBatch::new(vec![snapshot.clone(), snapshot.clone()])).await.is_err());
    assert!(backend.read_snapshot(NonZeroUsize::new(4096).unwrap(), memory.clone()).await.unwrap().is_none());
    backend.write_snapshot(StorageBatch::new(vec![snapshot.clone()])).await.unwrap();
    assert_eq!(backend.read_snapshot(NonZeroUsize::new(4096).unwrap(), memory.clone()).await.unwrap(), Some(snapshot));
    backend.close().await.unwrap();
}
