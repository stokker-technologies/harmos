/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Verify Durable Typed Replay And Exclusive JSON Root Ownership.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::{NonZeroU32, NonZeroUsize},
    path::Path,
};

// Third Party Imports

// Local Imports
use harmos_storage::{query::RecordSelection, *};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn backend(root: &Path) -> JsonStorage {
    let history = StoreHistory { store: Identifier::new("json-test").unwrap(), epoch: Identifier::new("first").unwrap() };
    let settings = JsonStorageSettings { root: root.to_owned(), max_entry_bytes: NonZeroUsize::new(4096).unwrap() };
    JsonStorage::new(settings, history)
}

fn record(history: &StoreHistory, sequence: u64) -> StoredRecord {
    let schema = SchemaKey {
        id: Identifier::new("record").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    StoredRecord {
        position: StorageCursor { history: history.clone(), sequence },
        schema,
        scope: None,
        item: Identifier::new("item").unwrap(),
        source: None,
        source_time: None,
        payload: vec![sequence as u8],
    }
}

/* === Tests === */

#[tokio::test]
async fn typed_replay() {
    let directory = tempfile::tempdir().unwrap();
    let storage = backend(directory.path());
    assert_eq!(storage.open().await.unwrap().sequence, 0);
    let first = record(storage.history(), 1);
    let batch = StorageBatch::new(vec![first.clone()]);
    assert_eq!(storage.write_records(batch.clone()).await.unwrap(), first.position);
    assert_eq!(storage.write_records(batch).await.unwrap(), first.position);

    let mut conflict = first.clone();
    conflict.payload = vec![99];
    assert!(storage.write_records(StorageBatch::new(vec![conflict])).await.is_err());
    let skipped = record(storage.history(), 3);
    assert!(storage.write_records(StorageBatch::new(vec![skipped])).await.is_err());
    storage.close().await.unwrap();

    let pending = directory.path().join("data").join("uncommitted.pending");
    std::fs::write(pending, b"interrupted write").unwrap();

    let reopened = backend(directory.path());
    assert_eq!(reopened.open().await.unwrap(), first.position);
    let memory = crate::manager::backend::read_memory().await;
    let snapshot = reopened.open_snapshot(&first.position, memory.clone()).await.unwrap();
    let selection = RecordSelection { schema: first.schema.clone(), scope: None, item: None, source_start: None, source_end: None };
    let page = ScanPage { after: 0, rows: NonZeroUsize::new(1).unwrap(), bytes: NonZeroUsize::new(4096).unwrap() };
    assert_eq!(reopened.read_records(&snapshot, &selection, page, memory.clone()).await.unwrap(), vec![first]);
    reopened.release_snapshot(&snapshot).await.unwrap();
    assert!(reopened.read_records(&snapshot, &selection, page, memory.clone()).await.is_err());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn root_lease() {
    let directory = tempfile::tempdir().unwrap();
    let first = backend(directory.path());
    let second = backend(directory.path());
    first.open().await.unwrap();
    assert!(second.open().await.is_err());
    first.close().await.unwrap();
    assert_eq!(second.open().await.unwrap().sequence, 0);
    second.close().await.unwrap();
}

#[tokio::test]
async fn truncated_commit() {
    let directory = tempfile::tempdir().unwrap();
    let storage = backend(directory.path());
    storage.open().await.unwrap();
    let row = record(storage.history(), 1);
    storage.write_records(StorageBatch::new(vec![row])).await.unwrap();
    storage.close().await.unwrap();

    let data = directory.path().join("data");
    let entry = std::fs::read_dir(data).unwrap().next().unwrap().unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(entry.path()).unwrap();
    let length = file.metadata().unwrap().len();
    file.set_len(length - 1).unwrap();
    drop(file);

    let reopened = backend(directory.path());
    assert!(reopened.open().await.is_err());
}

#[tokio::test]
async fn decode_requires_admission() {
    use crate::manager::backend::{Data, Recovery, history, storage_settings};
    use sha2::Digest;
    use std::sync::Arc;
    let directory = tempfile::tempdir().unwrap();
    let storage = backend(directory.path());
    storage.open().await.unwrap();
    let mut row = record(storage.history(), 1);
    row.payload = vec![7; 768];
    storage.write_records(StorageBatch::new(vec![row.clone()])).await.unwrap();
    let generous = crate::manager::backend::read_memory().await;
    let snapshot = storage.open_snapshot(&row.position, generous.clone()).await.unwrap();
    let selection = RecordSelection { schema: row.schema.clone(), scope: None, item: None, source_start: None, source_end: None };
    let page = ScanPage { after: 0, rows: NonZeroUsize::new(1).unwrap(), bytes: NonZeroUsize::new(4096).unwrap() };
    let decoded = storage.read_records(&snapshot, &selection, page, generous.clone()).await.unwrap();
    assert_eq!(decoded, vec![row.clone()]);
    assert!(decoded.memory_size() <= generous.bytes());
    drop(decoded);

    // Preserve Framing Integrity While Making Typed Byte Decoding Invalid.
    let mut value = serde_json::to_value(&row).unwrap();
    value["payload"] = serde_json::to_value(vec![256_u16; 768]).unwrap();
    let body = serde_json::to_vec(&value).unwrap();
    let digest: [u8; 32] = sha2::Sha256::digest(&body).into();
    let digits = b"0123456789abcdef";
    let mut checksum = Vec::with_capacity(64);
    for byte in digest {
        checksum.push(digits[(byte >> 4) as usize]);
        checksum.push(digits[(byte & 15) as usize]);
    }
    let segment = std::fs::read_dir(directory.path().join("data")).unwrap().next().unwrap().unwrap().path();
    let original = std::fs::read(&segment).unwrap();
    let header = original.iter().position(|byte| *byte == b'\n').unwrap() + 1;
    let mut changed = original[..header].to_vec();
    changed.extend_from_slice(b"{\"value\":");
    changed.extend_from_slice(&body);
    changed.extend_from_slice(b",\"sha256\":\"");
    changed.extend_from_slice(&checksum);
    changed.extend_from_slice(b"\"}\n");
    std::fs::write(segment, changed).unwrap();

    let mut settings = storage_settings();
    settings.budget.read_memory_bytes = NonZeroUsize::new(16 * 1024).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(4096).unwrap();
    let admission = StorageManager::<()>::new(
        StorageBackends { primary: Arc::new(Data::<()>::new(history())), state: Arc::new(Recovery::new(history())) },
        settings,
    )
    .unwrap();
    let limited = admission.read_memory(NonZeroUsize::MIN).await.unwrap();
    let refused = storage.read_records(&snapshot, &selection, page, limited.clone()).await.unwrap_err();
    assert!(matches!(refused, Error::ResourceExhausted { .. }));
    assert!(limited.bytes() <= 16 * 1024);

    let decoded = storage.read_records(&snapshot, &selection, page, generous.clone()).await.unwrap_err();
    assert!(matches!(decoded, Error::Backend { kind: BackendFailure::Corrupt, source, .. }
        if source.downcast_ref::<serde_json::Error>().is_some_and(serde_json::Error::is_data)));
    storage.release_snapshot(&snapshot).await.unwrap();
    storage.close().await.unwrap();
}
