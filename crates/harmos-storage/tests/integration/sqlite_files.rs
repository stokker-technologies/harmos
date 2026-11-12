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
use std::{
    num::NonZeroUsize,
    time::{Duration, UNIX_EPOCH},
};

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};
use harmos_storage::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */

#[tokio::test]
async fn finished_files_and_mutation_receipts_reopen_without_resetting_incompatible_roots() {
    // Persist Ordered Chunks, Refuse Conflicting Replay And Verify Completion.
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("files.sqlite");
    let history = StoreHistory {
        store: Identifier::new("store").unwrap(),
        epoch: Identifier::new("epoch").unwrap(),
    };
    let settings = SqliteSettings {
        chunk_rows: nz(4),
        chunk_bytes: nz(65536),
    };
    let backend = SqliteBackend::new(&path, history.clone(), settings).unwrap();
    backend.open().await.unwrap();
    let content = b"abcdefgh";
    let sha256 = Sha256::digest(content).into();
    let metadata = FileMetadata {
        id: Identifier::new("file").unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: Some(content.len() as u64),
        expected_sha256: Some(sha256),
    };
    let file = backend.create_file(&metadata).await.unwrap();
    backend.write_file(&file, 0, StorageBatch::new(content[..3].to_vec())).await.unwrap();
    backend.write_file(&file, 3, StorageBatch::new(content[3..].to_vec())).await.unwrap();
    backend.write_file(&file, 3, StorageBatch::new(content[3..].to_vec())).await.unwrap();
    assert!(backend.write_file(&file, 3, StorageBatch::new(vec![0; content.len() - 3])).await.is_err());
    let cut = StorageCursor { history: history.clone(), sequence: 1 };
    let mut wrong = sha256;
    wrong[0] ^= 1;
    assert!(backend.finish_file(&file, &cut, content.len() as u64, wrong).await.is_err());
    let reference = backend.finish_file(&file, &cut, content.len() as u64, sha256).await.unwrap();
    backend.write_files(StorageBatch::new(vec![reference.clone()])).await.unwrap();
    backend.write_files(StorageBatch::new(vec![reference.clone()])).await.unwrap();
    assert_eq!(backend.finish_file(&file, &cut, content.len() as u64, sha256).await.unwrap(), reference);
    let saved = backend.open_snapshot(&cut).await.unwrap();
    assert_eq!(backend.read_file(&saved, &reference, 2, nz(4)).await.unwrap(), content[2..6]);
    assert!(backend.abort_file(&file).await.is_err());

    // Save And Replay One Durable Mutation Receipt.
    let receipt = MutationReceipt {
        mutation: MutationId {
            window: Identifier::new("window").unwrap(),
            key: Identifier::new("publish").unwrap(),
        },
        fingerprint: Sha256::digest(content).into(),
        expires_at: UNIX_EPOCH + Duration::from_secs(1000),
        value: MutationValue::Cursor(cut.clone()),
    };
    backend.save_mutations(StorageBatch::new(vec![receipt.clone()])).await.unwrap();
    backend.save_mutations(StorageBatch::new(vec![receipt.clone()])).await.unwrap();
    let mut conflict = receipt.clone();
    conflict.fingerprint[0] ^= 1;
    assert!(backend.save_mutations(StorageBatch::new(vec![conflict])).await.is_err());
    backend.release_snapshot(&saved).await.unwrap();
    backend.close().await.unwrap();
    drop(backend);

    // Refuse A Different History Before Modifying The Database.
    let bytes = std::fs::read(&path).unwrap();
    let mut different = history.clone();
    different.epoch = Identifier::new("different").unwrap();
    let error = SqliteBackend::new(&path, different, settings).err().unwrap();
    assert_eq!(error.backend_failure(), Some(BackendFailure::Corrupt));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let backend = SqliteBackend::new(&path, history.clone(), settings).unwrap();
    backend.open().await.unwrap();
    let snapshot = backend.open_snapshot(&cut).await.unwrap();
    assert_eq!(backend.read_file(&snapshot, &reference, 0, nz(content.len())).await.unwrap(), content);
    assert_eq!(backend.recover_mutations(ReadLimits { rows: nz(4), bytes: nz(4096) }).await.unwrap(), vec![receipt]);

    // Foreign Tokens At The Same History And Cut Must Still Refuse.
    let other = SqliteBackend::new(root.path().join("other.sqlite"), history.clone(), settings).unwrap();
    other.open().await.unwrap();
    let staged = other.create_file(&metadata).await.unwrap();
    other.write_file(&staged, 0, StorageBatch::new(content.to_vec())).await.unwrap();
    let published = other.finish_file(&staged, &cut, content.len() as u64, sha256).await.unwrap();
    other.write_files(StorageBatch::new(vec![published.clone()])).await.unwrap();
    let foreign = other.open_snapshot(&cut).await.unwrap();
    assert!(backend.read_file(&foreign, &reference, 0, nz(1)).await.is_err());
    assert!(other.read_file(&snapshot, &published, 0, nz(1)).await.is_err());
    other.release_snapshot(&foreign).await.unwrap();
    other.close().await.unwrap();
    backend.release_snapshot(&snapshot).await.unwrap();
    backend.close().await.unwrap();
    drop(other);
    drop(backend);

    // Preserve A Non-Harmos Database Byte For Byte Instead Of Resetting It.
    let incompatible = root.path().join("incompatible.sqlite");
    let connection = rusqlite::Connection::open(&incompatible).unwrap();
    connection.execute_batch("CREATE TABLE evidence(value BLOB); INSERT INTO evidence VALUES(x'010203');").unwrap();
    drop(connection);
    let bytes = std::fs::read(&incompatible).unwrap();
    let error = SqliteBackend::new(&incompatible, history, settings).err().unwrap();
    assert_eq!(error.backend_failure(), Some(BackendFailure::Corrupt));
    assert_eq!(std::fs::read(&incompatible).unwrap(), bytes);
}
