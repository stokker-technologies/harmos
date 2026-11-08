/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// State Journal Failure and Reopen Integration Story

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
    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory {
        store: Identifier::new("state.test").unwrap(),
        epoch: Identifier::new("epoch.one").unwrap(),
    };
    let settings = StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(4096).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap(),
    };
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
    let limits = ReadLimits {
        rows: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let page = backend.read_entries(&start, &through, limits).await.unwrap();
    assert_eq!(page.entries, entries);
    assert!(page.next.is_none());
    backend.close().await.unwrap();
}

#[tokio::test]
async fn snapshot_requires_singleton() {
    use harmos_storage::{StorageBatch, state::RecoverySnapshot};
    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory {
        store: Identifier::new("snapshot.test").unwrap(),
        epoch: Identifier::new("one").unwrap(),
    };
    let settings = StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(4096).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    let through = backend.open().await.unwrap();
    let snapshot = RecoverySnapshot {
        required_data: StorageCursor { history, sequence: 0 },
        through,
        payload: harmos_storage::state::SnapshotPayload::from_bytes(
            &vec![7],
            std::num::NonZeroUsize::new(65536).unwrap(),
            harmos_domain::state::Position::ORIGIN,
        )
        .unwrap(),
    };
    assert!(backend.write_snapshot(StorageBatch::new(Vec::new())).await.is_err());
    assert!(backend.write_snapshot(StorageBatch::new(vec![snapshot.clone(), snapshot.clone()])).await.is_err());
    assert!(backend.read_snapshot(NonZeroUsize::new(4096).unwrap()).await.unwrap().is_none());
    backend.write_snapshot(StorageBatch::new(vec![snapshot.clone()])).await.unwrap();
    assert_eq!(backend.read_snapshot(NonZeroUsize::new(4096).unwrap()).await.unwrap(), Some(snapshot));
    backend.close().await.unwrap();
}

#[tokio::test]
async fn interrupted_segments_preserve_checkpoint_and_journal() {
    use harmos_domain::state::Position;
    use harmos_storage::StorageBatch;
    use harmos_storage::state::{RecoverySnapshot, SnapshotPayload, SnapshotWriter};
    use std::io::{Read, Write};

    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory {
        store: Identifier::new("segmented").unwrap(),
        epoch: Identifier::new("one").unwrap(),
    };
    let settings = StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(4096).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(1024).unwrap(),
    };
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    backend.open().await.unwrap();
    let required_data = StorageCursor { history: history.clone(), sequence: 0 };
    let at = |sequence| RecoveryPosition { history: history.clone(), sequence };
    let first = RecoveryEntry {
        required_data: required_data.clone(),
        position: at(1),
        payload: 1u64.to_le_bytes().to_vec(),
    };
    backend.write_entries(StorageBatch::new(vec![first])).await.unwrap();
    let previous = RecoverySnapshot {
        required_data: required_data.clone(),
        through: at(1),
        payload: SnapshotPayload::from_bytes(&1u64.to_le_bytes(), NonZeroUsize::new(64).unwrap(), Position(1)).unwrap(),
    };
    backend.write_snapshot(StorageBatch::new(vec![previous.clone()])).await.unwrap();
    let entries: Vec<_> = (2u64..=3)
        .map(|sequence| RecoveryEntry {
            required_data: required_data.clone(),
            position: at(sequence),
            payload: sequence.to_le_bytes().to_vec(),
        })
        .collect();
    backend.write_entries(StorageBatch::new(entries.clone())).await.unwrap();
    backend.flush(&at(3)).await.unwrap();
    let journal = fs::read(root.path().join("state.jsonl")).unwrap();
    let checkpoint = fs::read(root.path().join("state.json")).unwrap();

    // The First Segment Fits; The Second Fails Its Encoded Physical Bound.
    let mut writer = SnapshotWriter::new(NonZeroUsize::new(1024).unwrap()).unwrap();
    writer.write_all(&[1; 10]).unwrap();
    writer.finish_segment();
    writer.write_all(&[2; 800]).unwrap();
    let next = RecoverySnapshot {
        required_data,
        through: at(3),
        payload: writer.finish(Position(3)).unwrap(),
    };
    assert!(backend.write_snapshot(StorageBatch::new(vec![next])).await.is_err());
    let pending = fs::read_to_string(root.path().join("state.json.pending")).unwrap();
    assert_eq!(pending.lines().count(), 2);
    assert_eq!(fs::read(root.path().join("state.json")).unwrap(), checkpoint);
    assert_eq!(fs::read(root.path().join("state.jsonl")).unwrap(), journal);
    backend.close().await.unwrap();

    // Reopen Ignores The Partial Replacement And Replays After The Previous Snapshot.
    let reopened = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    assert_eq!(reopened.open().await.unwrap(), at(3));
    let snapshot = reopened.read_snapshot(NonZeroUsize::new(1024).unwrap()).await.unwrap().unwrap();
    assert_eq!(snapshot, previous);
    let mut bytes = [0; 8];
    snapshot.payload.reader().unwrap().read_exact(&mut bytes).unwrap();
    let mut recovered = u64::from_le_bytes(bytes);
    let page = reopened
        .read_entries(
            &snapshot.through,
            &at(3),
            ReadLimits {
                rows: NonZeroUsize::new(2).unwrap(),
                bytes: NonZeroUsize::new(4096).unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(page.entries, entries);
    for entry in page.entries {
        recovered += u64::from_le_bytes(entry.payload.try_into().unwrap());
    }
    assert_eq!(recovered, (1u64..=3).sum::<u64>());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn segmented_manifest_rejects_missing_reordered_and_damaged_frames() {
    use harmos_domain::state::Position;
    use harmos_storage::StorageBatch;
    use harmos_storage::state::{RecoverySnapshot, SnapshotPayload};

    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory {
        store: Identifier::new("integrity").unwrap(),
        epoch: Identifier::new("one").unwrap(),
    };
    let settings = StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(4096).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    let through = backend.open().await.unwrap();
    let bytes: Vec<u8> = (0..1000).map(|index| (index / 64) as u8).collect();
    let payload = SnapshotPayload::from_bytes(&bytes, NonZeroUsize::new(64).unwrap(), Position::ORIGIN).unwrap();
    let snapshot = RecoverySnapshot {
        required_data: StorageCursor { history: history.clone(), sequence: 0 },
        through,
        payload,
    };
    backend.write_snapshot(StorageBatch::new(vec![snapshot.clone()])).await.unwrap();
    assert_eq!(backend.read_snapshot(NonZeroUsize::new(4096).unwrap()).await.unwrap(), Some(snapshot));
    backend.close().await.unwrap();
    let checkpoint = fs::read_to_string(root.path().join("state.json")).unwrap();
    let journal = fs::read(root.path().join("state.jsonl")).unwrap();
    for damage in ["missing", "reordered", "damaged", "uncommitted"] {
        let copy = tempfile::tempdir().unwrap();
        fs::write(copy.path().join("state.jsonl"), &journal).unwrap();
        let mut frames: Vec<String> = checkpoint.lines().map(str::to_owned).collect();
        match damage {
            "missing" => {
                frames.remove(2);
            }
            "reordered" => frames.swap(1, 2),
            "damaged" => frames[1] = frames[1].replacen('A', "B", 1),
            "uncommitted" => {
                frames.pop();
            }
            _ => unreachable!(),
        }
        let damaged = frames.join("\n") + "\n";
        fs::write(copy.path().join("state.json"), &damaged).unwrap();
        let backend = FileBackend::new(copy.path().to_owned(), history.clone(), settings);
        assert!(backend.open().await.is_err(), "{damage}");
        assert_eq!(fs::read_to_string(copy.path().join("state.json")).unwrap(), damaged);
        assert_eq!(fs::read(copy.path().join("state.jsonl")).unwrap(), journal);
    }
}

#[tokio::test]
async fn existing_single_payload_checkpoint_remains_readable() {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let root = tempfile::tempdir().unwrap();
    let history = StoreHistory {
        store: Identifier::new("existing").unwrap(),
        epoch: Identifier::new("one").unwrap(),
    };
    let settings = StorageStateSettings {
        max_entry_bytes: NonZeroUsize::new(4096).unwrap(),
        max_checkpoint_bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let backend = FileBackend::new(root.path().to_owned(), history.clone(), settings);
    let through = backend.open().await.unwrap();
    backend.close().await.unwrap();
    let legacy = serde_json::to_vec(&serde_json::json!({
        "required_data": StorageCursor { history: history.clone(), sequence: 0 },
        "through": through,
        "payload": "Y2hlY2twb2ludA"
    }))
    .unwrap();
    let digest: String = Sha256::digest(&legacy).iter().map(|byte| format!("{byte:02x}")).collect();
    let line = format!("{{\"value\":{},\"sha256\":\"{digest}\"}}\n", String::from_utf8(legacy).unwrap());
    fs::write(root.path().join("state.json"), line).unwrap();
    let reopened = FileBackend::new(root.path().to_owned(), history, settings);
    assert_eq!(reopened.open().await.unwrap(), through);
    let snapshot = reopened.read_snapshot(NonZeroUsize::new(4096).unwrap()).await.unwrap().unwrap();
    let mut bytes = Vec::new();
    snapshot.payload.reader().unwrap().read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"checkpoint");
    assert_eq!(snapshot.payload.manifest().position, None);
    reopened.close().await.unwrap();
}
