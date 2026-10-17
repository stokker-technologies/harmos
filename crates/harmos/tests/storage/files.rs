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
use sha2::{Digest, Sha256};

// Local Imports
use super::*;

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

fn metadata(id: &str, bytes: &[u8]) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(id).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: Some(bytes.len() as u64),
        expected_sha256: Some(Sha256::digest(bytes).into()),
    }
}

fn whole(file: &harmos::storage::FileReference) -> harmos::storage::ReadFile {
    harmos::storage::ReadFile {
        file: file.clone(),
        range: harmos::storage::ByteRange {
            offset: 0,
            limit: None,
        },
    }
}

#[tokio::test]
async fn file_chunks_bounded() {
    let mut policy = config();
    policy.response.batch_bytes = NonZeroUsize::new(31).unwrap();
    let f = Fixture::new(policy).await;
    let bytes: Vec<_> = (0..8192).map(|n| (n % 251) as u8).collect();
    let reference = f
        .storage
        .write_file(metadata("large", &bytes), &bytes)
        .await
        .unwrap();
    assert!(
        f.data
            .control
            .chunk_sizes
            .lock()
            .unwrap()
            .iter()
            .all(|n| *n <= 31)
    );
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    let mut actual = Vec::new();
    while let Some(batch) = reader.next().await.unwrap() {
        assert!(batch.bytes.len() <= 31);
        actual.extend_from_slice(&batch.bytes);
    }
    assert_eq!(actual, bytes);
    drop(reader);
    f.finish().await;
}

#[tokio::test]
async fn file_awaits_flush() {
    let f = Fixture::new(config()).await;
    let bytes = b"unsaved bytes";
    let reference = f
        .storage
        .write_file(metadata("unsaved", bytes), bytes)
        .await
        .unwrap();
    assert_eq!(f.storage.status().saved.sequence, 0);
    let mut durable = f
        .storage
        .query(harmos::storage::query::Query {
            at: f.storage.cursor_saved(),
            source: harmos::storage::query::Source::Files(harmos::storage::query::FileSelection {
                scope: None,
                item: None,
            }),
            transforms: Vec::new(),
            slice: harmos::storage::query::RowSlice::default(),
        })
        .await
        .unwrap();
    assert!(matches!(
        durable.next().await.unwrap(),
        Some(harmos::storage::query::QueryArrow::Schema(_))
    ));
    assert!(durable.next().await.unwrap().is_none());
    f.storage.flush().await.unwrap();
    assert_eq!(
        f.storage.status().saved.sequence,
        reference.position.sequence
    );
    drop(durable);
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn chunk_retries_offset() {
    let f = Fixture::new(config()).await;
    f.data.control.chunk_failures.store(1, Ordering::SeqCst);
    let bytes = b"one chunk";
    let reference = f
        .storage
        .write_file(metadata("retry", bytes), bytes)
        .await
        .unwrap();
    assert_eq!(
        f.data.control.chunk_offsets.lock().unwrap().as_slice(),
        &[0, 0]
    );
    assert_eq!(reference.length, bytes.len() as u64);
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().bytes, bytes);
    drop(reader);
    f.finish().await;
}

#[tokio::test]
async fn finish_retries_position() {
    let f = Fixture::new(config()).await;
    let bytes = b"finished once";
    let mut file = f
        .storage
        .create_file(metadata("finish", bytes))
        .await
        .unwrap();
    file.write_all(bytes).await.unwrap();
    // An Uncertain Publication Is Retried At The Same Position.
    f.data.control.finish_failures.store(1, Ordering::SeqCst);
    let reference = file.finish().await.unwrap();
    assert_eq!(reference.position.sequence, 1);
    assert!(
        f.data
            .control
            .finish_positions
            .lock()
            .unwrap()
            .iter()
            .all(|position| *position == 1)
    );
    assert_eq!(f.data.blobs.lock().unwrap().len(), 1);
    assert!(f.data.files.lock().unwrap().is_empty());
    file.abort().await.unwrap();
    assert_eq!(f.data.blobs.lock().unwrap().len(), 1);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_create_cleans() {
    let f = Fixture::new(config()).await;
    f.data.control.block_create.store(true, Ordering::SeqCst);
    {
        let create = f.storage.create_file(metadata("cancel", b""));
        tokio::pin!(create);
        tokio::select! {
            biased;
            _ = &mut create => panic!("creation must block"),
            _ = f.data.control.create_entered.notified() => {}
        }
    }
    f.data.control.block_create.store(false, Ordering::SeqCst);
    f.data.control.create_release.notify_one();
    f.storage.close().await.unwrap();
    assert!(f.data.staged.lock().unwrap().is_empty());
    assert!(f.data.files.lock().unwrap().is_empty());
    assert_eq!(f.storage.status().accepted.sequence, 0);
    f.finish().await;
}

#[tokio::test]
async fn abort_failure_retryable() {
    let f = Fixture::new(config()).await;
    let mut file = f
        .storage
        .create_file(metadata("abort", b"x"))
        .await
        .unwrap();
    f.data.control.fail_abort.store(true, Ordering::SeqCst);
    assert!(file.abort().await.is_err());
    assert!(f.storage.close().await.is_err());
    assert!(f.data.control.open.load(Ordering::SeqCst));
    f.data.control.fail_abort.store(false, Ordering::SeqCst);
    file.abort().await.unwrap();
    assert!(f.data.staged.lock().unwrap().is_empty());
    f.finish().await;
}

#[tokio::test]
async fn integrity_failure_unpublished() {
    let f = Fixture::new(config()).await;
    let mut file = f
        .storage
        .create_file(metadata("bad", b"right"))
        .await
        .unwrap();
    file.write_all(b"wrong").await.unwrap();
    assert!(matches!(
        file.finish().await,
        Err(Error::InvalidInput {
            field: "file integrity",
            ..
        })
    ));
    file.abort().await.unwrap();
    assert_eq!(f.storage.status().accepted.sequence, 0);
    assert!(f.data.files.lock().unwrap().is_empty());
    assert!(f.data.staged.lock().unwrap().is_empty());
    f.finish().await;
}

#[tokio::test]
async fn failed_chunk_retryable() {
    let f = Fixture::new(config()).await;
    let bytes = b"immutable bytes";
    let reference = f
        .storage
        .write_file(metadata("read", bytes), bytes)
        .await
        .unwrap();
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    f.data.control.corrupt_file.store(true, Ordering::SeqCst);
    assert!(reader.next().await.is_err());
    assert_eq!(reader.offset(), 0);
    f.data.control.corrupt_file.store(false, Ordering::SeqCst);
    assert_eq!(reader.next().await.unwrap().unwrap().bytes, bytes);
    drop(reader);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_chunk_aborts() {
    let f = Fixture::new(config()).await;
    let bytes = b"cancelled chunk";
    let mut file = f
        .storage
        .create_file(metadata("chunk", bytes))
        .await
        .unwrap();
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let write = file.write_all(bytes);
        tokio::pin!(write);
        tokio::select! {
            biased;
            _ = &mut write => panic!("chunk must block"),
            _ = f.data.control.chunk_entered.notified() => {}
        }
    }
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_one();
    file.abort().await.unwrap();
    assert!(f.data.staged.lock().unwrap().is_empty());
    assert!(f.data.files.lock().unwrap().is_empty());
    assert_eq!(f.storage.status().accepted.sequence, 0);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_finish_drained() {
    let f = Fixture::new(config()).await;
    let bytes = b"sealed bytes";
    let mut file = f
        .storage
        .create_file(metadata("sealed", bytes))
        .await
        .unwrap();
    file.write_all(bytes).await.unwrap();
    f.data.control.block_finish.store(true, Ordering::SeqCst);
    {
        let finish = file.finish();
        tokio::pin!(finish);
        tokio::select! {
            biased;
            _ = &mut finish => panic!("finish must block"),
            _ = f.data.control.finish_entered.notified() => {}
        }
    }
    assert!(file.abort().await.is_err());
    drop(file);
    f.data.control.block_finish.store(false, Ordering::SeqCst);
    f.data.control.finish_release.notify_one();
    f.storage.close().await.unwrap();
    assert_eq!(f.storage.status().saved.sequence, 1);
    assert_eq!(f.data.files.lock().unwrap().len(), 1);
    assert!(f.data.staged.lock().unwrap().is_empty());
    f.finish().await;
}

#[tokio::test]
async fn budget_bounds_chunks() {
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(20_000).unwrap();
    let f = Fixture::new(policy).await;
    let bytes = vec![42; 100_000];
    let mut file = f
        .storage
        .create_file(metadata("bounded", &bytes))
        .await
        .unwrap();
    let base = f.storage.status().pending_bytes;
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let write = file.write_all(&bytes);
        tokio::pin!(write);
        tokio::select! {
            biased;
            _ = &mut write => panic!("chunk must block"),
            _ = f.data.control.chunk_entered.notified() => {}
        }
        assert!(f.storage.status().pending_bytes <= 20_000);
        assert!(f.data.control.chunk_sizes.lock().unwrap()[0] <= 20_000 - base);
        f.data.control.block_chunk.store(false, Ordering::SeqCst);
        f.data.control.chunk_release.notify_one();
        write.await.unwrap();
    }
    assert_eq!(file.length(), bytes.len() as u64);
    file.finish().await.unwrap();
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn stages_flush_immediately() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let f = Fixture::new(policy).await;
    // The Paused Clock Proves No Stage Waits For The Periodic Flush Timer.
    let started = tokio::time::Instant::now();
    write(&f.storage, "before-create").await;
    let mut file = f
        .storage
        .create_file(metadata("file", b"body"))
        .await
        .unwrap();
    assert_eq!(f.storage.status().saved.sequence, 1);
    write(&f.storage, "before-chunk").await;
    file.write_all(b"body").await.unwrap();
    assert_eq!(f.storage.status().saved.sequence, 2);
    write(&f.storage, "before-finish").await;
    file.finish().await.unwrap();
    assert_eq!(f.storage.status().accepted.sequence, 4);
    assert_eq!(f.storage.status().saved.sequence, 3);
    // The Periodic Flush Interval Is 100 ms.
    assert!(started.elapsed() < Duration::from_millis(100));
    f.finish().await;
}

#[tokio::test]
async fn forged_reference_refused() {
    let f = Fixture::new(config()).await;
    let file = f
        .storage
        .write_file(metadata("verified", b"abc"), b"abc")
        .await
        .unwrap();
    for saved in [false, true] {
        if saved {
            f.storage.flush().await.unwrap();
        }
        for field in 0..6 {
            let mut forged = file.clone();
            match field {
                0 => forged.length += 1,
                1 => forged.sha256[0] ^= 1,
                2 => forged.metadata.id = Identifier::new("other").unwrap(),
                3 => forged.position.sequence += 1,
                4 => forged.position.history.epoch = Identifier::new("other").unwrap(),
                _ => forged.position.sequence = 0,
            }
            assert!(
                f.storage.read_file(whole(&forged)).await.is_err(),
                "saved={saved}, field={field}"
            );
        }
        let mut reader = f.storage.read_file(whole(&file)).await.unwrap();
        assert_eq!(reader.next().await.unwrap().unwrap().bytes, b"abc");
        assert_eq!(f.storage.status().saved.sequence, u64::from(saved));
    }
    f.finish().await;
}

#[tokio::test]
async fn reader_keeps_reference() {
    let f = Fixture::new(config()).await;
    f.data.control.block_release.store(true, Ordering::SeqCst);
    let file = f
        .storage
        .write_file(metadata("owned", b"abc"), b"abc")
        .await
        .unwrap();
    let mut reader = f.storage.read_file(whole(&file)).await.unwrap();
    f.storage.flush().await.unwrap();
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert!(f.storage.status().read_bytes > 0);
    assert_eq!(reader.next().await.unwrap().unwrap().bytes, b"abc");
    assert!(reader.next().await.unwrap().is_none());
    // The Exhausted Reader Releases Its Backend Snapshot.
    f.data.control.release_entered.notified().await;
    f.data.control.block_release.store(false, Ordering::SeqCst);
    f.data.control.release_release.notify_one();
    // The exhausted reader still owns the immutable reference exposed by reference().
    assert_eq!(reader.reference(), &file);
    assert!(f.storage.status().read_bytes > 0);
    drop(reader);
    assert_eq!(f.storage.status().read_bytes, 0);
    f.finish().await;
}
