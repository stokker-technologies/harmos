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
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::{Fixture, config};
use harmos_runtime::Identifier;
use harmos_runtime::storage::operations::DeleteFile;
use harmos_runtime::storage::{FileMetadata, MutationId, StagedFile, StorageBackend};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn metadata(id: &str) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(id).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    }
}
fn request(f: &Fixture, id: &str) -> DeleteFile {
    DeleteFile {
        mutation: MutationId { window: f.storage.mutation_window().unwrap().id, key: Identifier::new("delete").unwrap() },
        file: StagedFile { history: f.data.history.clone(), id: Identifier::new(id).unwrap() },
    }
}

/* === Tests === */

#[tokio::test]
async fn delete_removes_file() {
    let f = Fixture::new(config()).await;
    let mut producer = f.storage.produce_file(metadata("terminal")).await.unwrap();
    producer.write_all(b"abcdef").await.unwrap();
    let request = request(&f, "terminal");
    assert!(f.storage.delete_file_once(&request.mutation, &request.file).await.is_err());
    producer.finish().await.unwrap();
    let mut reader = f.storage.follow_file(&request.file, 0).await.unwrap();
    let accepted = reader.next_batch(NonZeroUsize::new(2).unwrap()).await.unwrap().unwrap();
    let deleted = f.storage.delete_file_once(&request.mutation, &request.file).await.unwrap();
    assert_eq!(deleted, request.file);
    assert_eq!(accepted.bytes, b"ab");
    assert!(reader.next_batch(NonZeroUsize::MIN).await.is_err());
    assert!(f.storage.follow_file(&request.file, 0).await.is_err());
    assert_eq!(f.storage.delete_file_once(&request.mutation, &request.file).await.unwrap(), deleted);
    let mut query = f
        .storage
        .query(harmos_runtime::storage::operations::Query {
            at: f.storage.cursor_saved(),
            source: harmos_runtime::storage::query::Source::Files(harmos_runtime::storage::query::FileSelection {
                scope: None,
                item: None,
            }),
            transforms: Vec::new(),
            slice: harmos_runtime::storage::query::RowSlice::default(),
        })
        .await
        .unwrap();
    assert!(matches!(query.next().await.unwrap(), Some(harmos_runtime::storage::query::QueryArrow::Schema(_))));
    assert!(query.next().await.unwrap().is_none());
    drop(query);
    assert!(f.data.files.lock().unwrap().is_empty());
    assert!(f.data.blobs.lock().unwrap().is_empty());
    assert!(f.data.create_file(&metadata("terminal")).await.is_err());
    let other = StagedFile { id: Identifier::new("other").unwrap(), ..request.file.clone() };
    assert!(f.storage.delete_file_once(&request.mutation, &other).await.is_err());
    drop(accepted);
    drop(reader);
    f.finish().await;
}

#[tokio::test]
async fn delete_abandoned_missing() {
    let f = Fixture::new(config()).await;
    let mut producer = f.storage.produce_file(metadata("partial")).await.unwrap();
    producer.write_all(b"partial").await.unwrap();
    producer.abort().await.unwrap();
    let mut request = request(&f, "partial");
    f.storage.delete_file_once(&request.mutation, &request.file).await.unwrap();
    assert!(f.data.staged.lock().unwrap().is_empty());
    request.mutation.key = Identifier::new("missing").unwrap();
    request.file.id = Identifier::new("missing").unwrap();
    f.storage.delete_file_once(&request.mutation, &request.file).await.unwrap();
    assert!(f.data.create_file(&metadata("missing")).await.is_err());
    f.finish().await;
}

#[tokio::test]
async fn cancelled_delete_settles() {
    let f = Fixture::new(config()).await;
    f.storage.write_file(metadata("cancelled"), b"bytes").await.unwrap();
    let request = request(&f, "cancelled");
    f.data.control.block_delete.store(true, Ordering::SeqCst);
    let mut deletion = Box::pin(f.storage.delete_file_once(&request.mutation, &request.file));
    tokio::select! {
        biased;
        _ = &mut deletion => panic!("deletion must wait"),
        _ = f.data.control.delete_entered.notified() => {}
    }
    drop(deletion);
    f.data.control.block_delete.store(false, Ordering::SeqCst);
    f.data.control.delete_release.notify_one();
    f.storage.delete_file_once(&request.mutation, &request.file).await.unwrap();
    assert!(f.data.mutation_receipts.lock().unwrap().contains_key(&request.mutation));
    assert_eq!(f.storage.status().read_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn deletion_preserves_other() {
    let f = Fixture::new(config()).await;
    let kept = f.storage.write_file(metadata("kept-first"), b"kept").await.unwrap();
    let removed = f.storage.write_file(metadata("removed-last"), b"gone").await.unwrap();
    f.storage.flush().await.unwrap();
    let whole = |file: &harmos_runtime::storage::FileReference| harmos_runtime::storage::operations::ReadFile {
        file: file.clone(),
        range: harmos_runtime::storage::ByteRange { offset: 0, limit: None },
    };
    let mut pinned = f.storage.read_file(whole(&removed)).await.unwrap();
    let request = request(&f, "removed-last");
    f.storage.delete_file_once(&request.mutation, &request.file).await.unwrap();
    assert!(pinned.next().await.is_err());
    assert!(f.storage.read_file(whole(&removed)).await.is_err());
    let mut reader = f.storage.read_file(whole(&kept)).await.unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().bytes, b"kept");
    drop((pinned, reader));
    f.finish().await;
}

#[tokio::test]
async fn deleted_checkpoint_refused() {
    let f = Fixture::new(config()).await;
    let mut producer = f.storage.produce_file(metadata("checkpoint")).await.unwrap();
    producer.write_all(b"partial").await.unwrap();
    f.storage.flush().await.unwrap();
    let checkpoint = f.data.saved_productions.lock().unwrap()["checkpoint"].0.clone();
    producer.abort().await.unwrap();
    let request = request(&f, "checkpoint");
    f.storage.delete_file_once(&request.mutation, &request.file).await.unwrap();
    f.data.save_production(&checkpoint).await.unwrap();
    assert!(f.data.saved_productions.lock().unwrap().is_empty());
    assert!(f.data.staged.lock().unwrap().is_empty());
    f.finish().await;
}

#[tokio::test]
async fn close_settles_deletion() {
    let f = Fixture::new(config()).await;
    f.storage.write_file(metadata("close"), b"bytes").await.unwrap();
    let request = request(&f, "close");
    f.data.control.block_delete.store(true, Ordering::SeqCst);
    let mut deletion = Box::pin(f.storage.delete_file_once(&request.mutation, &request.file));
    tokio::select! {
        biased;
        _ = &mut deletion => panic!("deletion must wait"),
        _ = f.data.control.delete_entered.notified() => {}
    }
    drop(deletion);
    let closing = f.storage.close();
    tokio::pin!(closing);
    tokio::select! {
        biased;
        result = &mut closing => panic!("close must wait for the blocked deletion: {result:?}"),
        // One poll of close: it must already observe the in-flight deletion.
        _ = std::future::ready(()) => {}
    }
    assert!(f.storage.status().closing);
    f.data.control.block_delete.store(false, Ordering::SeqCst);
    f.data.control.delete_release.notify_one();
    closing.await.unwrap();
    assert!(f.data.mutation_receipts.lock().unwrap().contains_key(&request.mutation));
    assert!(f.data.blobs.lock().unwrap().is_empty());
    assert_eq!(f.storage.status().read_bytes, 0);
    f.runtime.stop().await.unwrap();
}
