/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::future::Future;

use sha2::{Digest, Sha256};

use super::*;

fn metadata(id: &str, bytes: &[u8]) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(id).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: Some(bytes.len() as u64),
        expected_sha256: Some(Sha256::digest(bytes).into()),
    }
}

fn whole(file: &harmos_storage::FileReference) -> harmos_storage::FileRead {
    harmos_storage::FileRead {
        source: harmos_domain::model::storage::files::FileSource::Stored(file.clone()),
        range: harmos_storage::ByteRange { offset: 0, limit: None },
    }
}

fn shareable_tempfile() -> (tempfile::NamedTempFile, std::fs::File) {
    let owner = tempfile::NamedTempFile::new().unwrap();
    let file = owner.as_file().try_clone().unwrap();
    (owner, file)
}

#[tokio::test(start_paused = true)]
async fn file_roundtrip_recovers() {
    let large = (0..8192).map(|n| (n % 251) as u8).collect::<Vec<_>>();
    let cases =
        [("large", large), ("unsaved", b"unsaved bytes".to_vec()), ("retry", b"one chunk".to_vec()), ("finish", b"finished once".to_vec())];
    for (id, bytes) in cases {
        let mut policy = config();
        policy.response.batch_bytes = NonZeroUsize::new(31).unwrap();
        let f = Fixture::new(policy).await;
        let mut file = f.storage.create_file(metadata(id, &bytes)).await.unwrap();
        f.data.control.chunk_failures.store(1, Ordering::SeqCst);
        file.write_all((bytes).to_vec()).await.unwrap();
        let offsets = f.data.control.chunk_offsets.lock().unwrap().clone();
        assert_eq!(&offsets[..2], &[0, 0]);
        if bytes.len() <= 31 {
            assert_eq!(offsets, [0, 0]);
        }
        assert!(f.data.control.chunk_sizes.lock().unwrap().iter().all(|n| *n <= 31));

        // An Uncertain Publication Reuses Its Position Without Duplicating The Blob.
        f.data.control.finish_failures.store(1, Ordering::SeqCst);
        let reference = file.finish().await.unwrap();
        assert_eq!(reference.position.sequence, 1);
        assert_eq!(reference.length, bytes.len() as u64);
        assert!(f.data.control.finish_positions.lock().unwrap().iter().all(|position| *position == 1));
        assert_eq!(f.data.blobs.lock().unwrap().len(), 1);
        assert!(f.data.files.lock().unwrap().is_empty());
        file.abort().await.unwrap();
        assert_eq!(f.data.blobs.lock().unwrap().len(), 1);

        // Bounded Reads Reconstruct The Exact Published Bytes.
        let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
        let first = reader.next().await.unwrap().unwrap();
        assert!(first.bytes().len() <= 31);
        assert_eq!(first.bytes(), &bytes[..first.bytes().len()]);
        if bytes.len() <= 31 {
            assert_eq!(first.bytes(), bytes);
        }
        let mut actual = first.bytes().to_vec();
        while let Some(batch) = reader.next().await.unwrap() {
            assert!(batch.bytes().len() <= 31);
            actual.extend_from_slice(batch.bytes());
        }
        assert_eq!(actual, bytes);
        drop(reader);

        // Logical Publication Remains Separate From Durable Query Visibility.
        assert_eq!(f.storage.status().saved.sequence, 0);
        let mut durable = f
            .storage
            .query(harmos_storage::Query {
                at: f.storage.cursor_saved(),
                source: harmos_storage::query::Source::Files(harmos_storage::query::FileSelection { scope: None, item: None }),
                transforms: Vec::new(),
                slice: harmos_storage::query::RowSlice::default(),
            })
            .await
            .unwrap();
        assert!(matches!(durable.next().await.unwrap(), Some(harmos_storage::query::QueryArrow::Schema(_))));
        assert!(durable.next().await.unwrap().is_none());
        f.storage.flush().await.unwrap();
        assert_eq!(f.storage.status().saved.sequence, reference.position.sequence);
        drop(durable);
        f.finish().await;
    }
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
    let mut file = f.storage.create_file(metadata("abort", b"x")).await.unwrap();
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
    let mut file = f.storage.create_file(metadata("bad", b"right")).await.unwrap();
    file.write_all((b"wrong").to_vec()).await.unwrap();
    assert!(matches!(file.finish().await, Err(harmos_storage::Error::InvalidInput { field: "file integrity", .. })));
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
    let reference = f.storage.write_file(metadata("read", bytes), (bytes).to_vec()).await.unwrap();
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    f.data.control.corrupt_file.store(true, Ordering::SeqCst);
    assert!(reader.next().await.is_err());
    assert_eq!(reader.offset(), 0);
    f.data.control.corrupt_file.store(false, Ordering::SeqCst);
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), bytes);
    drop(reader);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_chunk_aborts() {
    let f = Fixture::new(config()).await;
    let bytes = b"cancelled chunk";
    let mut file = f.storage.create_file(metadata("chunk", bytes)).await.unwrap();
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let write = file.write_all((bytes).to_vec());
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
    let mut file = f.storage.create_file(metadata("sealed", bytes)).await.unwrap();
    file.write_all((bytes).to_vec()).await.unwrap();
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
    let mut file = f.storage.create_file(metadata("bounded", &bytes)).await.unwrap();
    let base = f.storage.status().pending_bytes;
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let write = file.write_all((bytes).to_vec());
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

#[tokio::test]
async fn retained_write_waits() {
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(4096).unwrap();
    let f = Fixture::new(policy).await;
    let full = vec![42; 4096];
    let mut first = f.storage.create_file(metadata("full", &full)).await.unwrap();
    let tail = b"retained";
    let mut second = f.storage.produce_file(metadata("waiting", tail)).await.unwrap();
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let append = first.write_all(full.clone());
        tokio::pin!(append);
        tokio::select! {
            biased;
            result = &mut append => panic!("the full-budget append must block: {result:?}"),
            _ = f.data.control.chunk_entered.notified() => {}
        }
        assert_eq!(f.storage.status().pending_bytes, 4096);
        let retained = second.write_retained_at(0, harmos_storage::FileChunk::new(tail.to_vec()));
        tokio::pin!(retained);
        assert!(std::future::poll_fn(|cx| std::task::Poll::Ready(retained.as_mut().poll(cx).is_pending())).await);
        f.data.control.block_chunk.store(false, Ordering::SeqCst);
        f.data.control.chunk_release.notify_one();
        let (written, copied) = tokio::join!(append, retained);
        written.unwrap();
        copied.unwrap();
    }
    assert_eq!(first.length(), full.len() as u64);
    assert_eq!(second.length(), tail.len() as u64);
    first.finish().await.unwrap();
    let reference = second.finish().await.unwrap();
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), tail);
    drop(reader);
    f.finish().await;
}

#[tokio::test]
async fn retained_append_retries() {
    let f = Fixture::new(config()).await;
    let bytes = b"retained failed bytes";
    let mut file = f.storage.produce_file(metadata("retained-retry", bytes)).await.unwrap();
    *f.data.control.permanent_chunk_file.lock().unwrap() = Some("retained-retry".into());
    assert!(file.write_retained_at(0, harmos_storage::FileChunk::new(bytes.to_vec())).await.is_err());
    assert_eq!(file.length(), 0);
    assert!(file.write_retained_at(1, harmos_storage::FileChunk::new(bytes.to_vec())).await.is_err());
    assert!(file.write_retained_at(0, harmos_storage::FileChunk::new(b"changed bytes".to_vec())).await.is_err());
    *f.data.control.permanent_chunk_file.lock().unwrap() = None;
    let retried = file.retry_append().await.unwrap().unwrap();
    assert_eq!(retried.as_slice(), bytes);
    assert_eq!(*f.data.control.chunk_offsets.lock().unwrap(), [0, 0]);
    let reference = file.finish().await.unwrap();
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), bytes);
    drop(reader);
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
    let mut file = f.storage.create_file(metadata("file", b"body")).await.unwrap();
    assert_eq!(f.storage.status().saved.sequence, 1);
    write(&f.storage, "before-chunk").await;
    file.write_all((b"body").to_vec()).await.unwrap();
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
    let file = f.storage.write_file(metadata("verified", b"abc"), (b"abc").to_vec()).await.unwrap();
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
            assert!(f.storage.read_file(whole(&forged)).await.is_err(), "saved={saved}, field={field}");
        }
        let mut reader = f.storage.read_file(whole(&file)).await.unwrap();
        assert_eq!(reader.next().await.unwrap().unwrap().bytes(), b"abc");
        assert_eq!(f.storage.status().saved.sequence, u64::from(saved));
    }
    f.finish().await;
}

#[tokio::test]
async fn reader_keeps_metadata() {
    let f = Fixture::new(config()).await;
    f.data.control.block_release.store(true, Ordering::SeqCst);
    let file = f.storage.write_file(metadata("owned", b"abc"), (b"abc").to_vec()).await.unwrap();
    let mut reader = f.storage.read_file(whole(&file)).await.unwrap();
    f.storage.flush().await.unwrap();
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), b"abc");
    assert!(reader.next().await.unwrap().is_none());
    // The Exhausted Reader Schedules Backend Snapshot Release.
    f.data.control.release_entered.notified().await;
    // Completion Remains Inspectable While That Release Is Pending.
    assert_eq!(reader.completion().unwrap().bytes, file.length);
    drop(reader);
    f.data.control.block_release.store(false, Ordering::SeqCst);
    f.data.control.release_release.notify_one();
    f.data.snapshots_released().await;
    f.finish().await;
}

async fn wait_chunks(control: &super::backend::Control, count: usize) {
    loop {
        let changed = control.chunk_changed.notified();
        if control.chunk_active.lock().unwrap().values().sum::<usize>() >= count {
            return;
        }
        changed.await;
    }
}

#[tokio::test]
async fn bounded_file_parallelism() {
    let mut policy = config();
    policy.request.max_active_file_operations = NonZeroUsize::new(2).unwrap();
    policy.response.batch_bytes = NonZeroUsize::new(3).unwrap();
    let f = Fixture::new(policy).await;
    let mut producers = Vec::new();
    for id in 0..3 {
        producers.push(f.storage.create_file(metadata(&format!("parallel-{id}"), b"abcdef")).await.unwrap());
    }
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    let writes: Vec<_> = producers
        .into_iter()
        .map(|mut file| {
            tokio::spawn(async move {
                file.write_all(b"abcdef".to_vec()).await.unwrap();
                file.finish().await.unwrap()
            })
        })
        .collect();
    wait_chunks(&f.data.control, 2).await;
    assert_eq!(f.data.control.chunk_active.lock().unwrap().values().sum::<usize>(), 2);
    assert_eq!(f.data.control.chunk_pointers.lock().unwrap().len(), 2);
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_waiters();
    let mut positions = Vec::new();
    for write in writes {
        positions.push(write.await.unwrap().position.sequence);
    }
    positions.sort_unstable();
    assert_eq!(positions, [1, 2, 3]);
    assert_eq!(f.data.control.chunk_peak.load(Ordering::SeqCst), 2);
    assert!(f.data.control.chunk_peak_by_file.lock().unwrap().values().all(|peak| *peak == 1));
    for id in 0..3 {
        let offsets: Vec<_> = f
            .data
            .control
            .chunk_pointers
            .lock()
            .unwrap()
            .iter()
            .filter(|(file, ..)| file == &format!("parallel-{id}"))
            .map(|(_, offset, ..)| *offset)
            .collect();
        assert_eq!(offsets, [0, 3]);
    }
    assert_eq!(f.storage.flush().await.unwrap().sequence, 3);
    f.finish().await;
}

#[tokio::test]
async fn pressure_preserves_owner() {
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(40_000).unwrap();
    policy.request.max_pending_write_batches = NonZeroUsize::MIN;
    policy.response.batch_bytes = NonZeroUsize::new(1024).unwrap();
    let f = Fixture::new(policy).await;
    let bytes = vec![42; 16_000];
    let pointer = bytes.as_ptr() as usize;
    let capacity = bytes.capacity();
    let mut file = f.storage.create_file(metadata("pressure", &bytes)).await.unwrap();
    f.data.control.block_write.store(true, Ordering::SeqCst);
    f.storage.write_records(vec![record(&schema(), &Identifier::new("queued").unwrap(), &vec![7; 16_000])]).await.unwrap();
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    let writing = tokio::spawn(async move {
        file.write_all(bytes).await.unwrap();
        file
    });
    f.data.control.write_entered.notified().await;
    assert!(f.data.control.chunk_pointers.lock().unwrap().is_empty());
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    f.data.control.chunk_entered.notified().await;
    let first = f.data.control.chunk_pointers.lock().unwrap()[0].clone();
    assert_eq!(first.2, pointer);
    assert_eq!(first.3, capacity);
    assert!(f.storage.status().pending_bytes <= 40_000);
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_waiters();
    let mut file = writing.await.unwrap();
    for (_, offset, address, held) in f.data.control.chunk_pointers.lock().unwrap().iter() {
        assert_eq!(*address, pointer + *offset as usize);
        assert_eq!(*held, capacity);
    }
    assert_eq!(file.length(), 16_000);
    let reference = file.finish().await.unwrap();
    assert_eq!(reference.sha256, <[u8; 32]>::from(Sha256::digest(vec![42; 16_000])));
    f.finish().await;
}

#[tokio::test]
async fn failure_keeps_peer() {
    let mut policy = config();
    policy.request.max_active_file_operations = NonZeroUsize::new(2).unwrap();
    let f = Fixture::new(policy).await;
    let mut slow = f.storage.produce_file(metadata("slow", b"abc")).await.unwrap();
    let mut failed = f.storage.produce_file(metadata("failed", b"abc")).await.unwrap();
    *f.data.control.block_chunk_file.lock().unwrap() = Some("slow".into());
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    let slow_write = tokio::spawn(async move {
        let result = slow.write_all(b"abc".to_vec()).await;
        (slow, result)
    });
    f.data.control.chunk_entered.notified().await;
    *f.data.control.permanent_chunk_file.lock().unwrap() = Some("failed".into());
    let failed_write = tokio::spawn(async move {
        let result = failed.write_all(b"abc".to_vec()).await;
        (failed, result)
    });
    let (mut failed, result) = failed_write.await.unwrap();
    assert!(result.is_err());
    assert!(f.storage.status().failure.is_some());
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_waiters();
    let (mut slow, result) = slow_write.await.unwrap();
    assert!(result.is_ok());
    assert_eq!(slow.length(), 3);
    assert!(f.storage.status().failure.is_some());
    assert!(f.storage.create_file(metadata("not-admitted", b"x")).await.is_err());
    failed.abort().await.unwrap();
    let reference = slow.finish().await.unwrap();
    assert_eq!(reference.sha256, <[u8; 32]>::from(Sha256::digest(b"abc")));
    f.finish().await;
}

#[tokio::test]
async fn close_drains_files() {
    let mut policy = config();
    policy.request.max_active_file_operations = NonZeroUsize::new(2).unwrap();
    let f = Fixture::new(policy).await;
    let mut first = f.storage.create_file(metadata("close-first", b"abc")).await.unwrap();
    let mut second = f.storage.create_file(metadata("close-second", b"abc")).await.unwrap();
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    let first = tokio::spawn(async move { first.write_all(b"abc".to_vec()).await });
    let second = tokio::spawn(async move { second.write_all(b"abc".to_vec()).await });
    wait_chunks(&f.data.control, 2).await;
    first.abort();
    second.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    assert!(second.await.unwrap_err().is_cancelled());
    let closing_storage = f.storage.clone();
    let closing = closing_storage.close();
    tokio::pin!(closing);
    assert!(super::pending(closing.as_mut()).await);
    assert!(f.storage.status().closing);
    assert!(f.data.control.open.load(Ordering::SeqCst));
    assert_eq!(f.data.control.chunk_active.lock().unwrap().values().sum::<usize>(), 2);
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_waiters();
    closing.await.unwrap();
    assert!(f.data.control.chunk_active.lock().unwrap().is_empty());
    assert!(f.data.staged.lock().unwrap().is_empty());
    assert!(!f.data.control.open.load(Ordering::SeqCst));
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn file_bypasses_persistence() {
    let mut policy = config();
    policy.request.max_active_file_operations = NonZeroUsize::new(2).unwrap();
    let f = Fixture::new(policy).await;
    let mut file = f.storage.create_file(metadata("independent", b"abc")).await.unwrap();
    f.data.control.block_write.store(true, Ordering::SeqCst);
    write(&f.storage, "ordered").await;
    let storage = f.storage.clone();
    let flushing = tokio::spawn(async move { storage.flush().await });
    f.data.control.write_entered.notified().await;
    file.write_all(b"abc".to_vec()).await.unwrap();
    assert_eq!(file.length(), 3);
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    assert!(!flushing.is_finished());
    {
        let finishing = file.finish();
        tokio::pin!(finishing);
        assert!(super::pending(finishing.as_mut()).await);
        assert!(f.data.control.finish_positions.lock().unwrap().is_empty());
        f.data.control.block_write.store(false, Ordering::SeqCst);
        f.data.control.write_release.notify_one();
        assert_eq!(flushing.await.unwrap().unwrap().sequence, 1);
        assert_eq!(finishing.await.unwrap().position.sequence, 2);
    }
    f.finish().await;
}

#[tokio::test]
async fn file_projection_retains() {
    let f = Fixture::new(config()).await;
    let mut file = f.storage.create_file(metadata("projection", b"abc")).await.unwrap();
    file.write_all(b"abc".to_vec()).await.unwrap();
    let reference = file.finish().await.unwrap();
    f.storage.flush().await.unwrap();
    let session = f.storage.clone();
    let mut reader = session.read_file(whole(&reference)).await.unwrap();
    let batch = reader.next().await.unwrap().unwrap();
    let pointer = batch.bytes().as_ptr();
    let projected = batch.try_map(|batch| Ok::<_, Error>(batch.bytes)).unwrap();
    assert_eq!(projected.as_ptr(), pointer);
    drop(reader);
    drop(session);
    f.data.snapshots_released().await;
    assert_eq!(projected.as_slice(), b"abc");
    drop(projected);
    f.finish().await;
}

#[tokio::test]
async fn file_slice_refuses() {
    let f = Fixture::new(config()).await;
    let mut file = f.storage.create_file(metadata("slice", b"abc")).await.unwrap();
    file.write_all(b"abc".to_vec()).await.unwrap();
    let reference = file.finish().await.unwrap();
    f.storage.flush().await.unwrap();
    let session = f.storage.clone();
    let mut reader = session.read_file(whole(&reference)).await.unwrap();
    let batch = reader.next().await.unwrap().unwrap();
    let slice = batch.slice(1..3);
    let called = std::cell::Cell::new(false);
    let result = batch.try_map(|batch| {
        called.set(true);
        Ok::<_, Error>(batch.bytes)
    });
    assert!(result.is_err());
    assert!(!called.get());
    drop(reader);
    drop(session);
    f.data.snapshots_released().await;
    assert_eq!(slice.bytes(), b"bc");
    drop(slice);
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_retains_cancelled_copy() {
    use std::io::Write;
    let f = Fixture::new(config()).await;
    let (_source_owner, mut source) = shareable_tempfile();
    let expected = b"original opened source";
    source.write_all(expected).unwrap();
    let target = f.storage.produce_file(metadata("opened-capture", expected)).await.unwrap();
    let mut capture = f.storage.capture_opened_file(source, target, ByteRange { offset: 0, limit: None }, CaptureConsistency::Stable);
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let finish = capture.finish();
        tokio::pin!(finish);
        tokio::select! {
            biased;
            _ = &mut finish => panic!("copy must block"),
            _ = f.data.control.chunk_entered.notified() => {}
        }
    }
    assert_eq!(capture.acknowledged(), 0);
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_waiters();
    let reference = capture.finish().await.unwrap();
    assert_eq!(capture.acknowledged(), expected.len() as u64);
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), expected);
    assert_eq!(capture.finish().await.unwrap(), reference);
    drop((reader, capture));
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_keeps_source_identity() {
    let f = Fixture::new(config()).await;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source");
    std::fs::write(&path, b"original").unwrap();
    let source = std::fs::File::open(&path).unwrap();
    std::fs::rename(&path, directory.path().join("retained")).unwrap();
    std::fs::write(&path, b"replaced").unwrap();
    let target = f.storage.produce_file(metadata("identity-capture", b"original")).await.unwrap();
    let mut capture = f.storage.capture_opened_file(source, target, ByteRange { offset: 0, limit: None }, CaptureConsistency::Stable);
    let reference = capture.finish().await.unwrap();
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), b"original");
    drop((reader, capture));
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_retries_exact_failed_chunk() {
    use std::io::Write;
    let f = Fixture::new(config()).await;
    let (_source_owner, mut source) = shareable_tempfile();
    let expected = b"retained failed bytes";
    source.write_all(expected).unwrap();
    let target = f.storage.produce_file(metadata("failed-opened-capture", expected)).await.unwrap();
    let mut capture = f.storage.capture_opened_file(source, target, ByteRange { offset: 0, limit: None }, CaptureConsistency::Stable);
    *f.data.control.permanent_chunk_file.lock().unwrap() = Some("failed-opened-capture".into());
    assert!(capture.finish().await.is_err());
    assert_eq!(capture.acknowledged(), 0);
    *f.data.control.permanent_chunk_file.lock().unwrap() = None;
    let reference = capture.finish().await.unwrap();
    assert_eq!(reference.length, expected.len() as u64);
    assert_eq!(capture.acknowledged(), expected.len() as u64);
    drop(capture);
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_verifies_retained_prefix() {
    use std::io::Write;
    let f = Fixture::new(config()).await;
    for (id, source_bytes, succeeds) in
        [("matching-prefix", b"prefixsuffix".as_slice(), true), ("differing-prefix", b"badfixsuffix".as_slice(), false)]
    {
        let (_source_owner, mut source) = shareable_tempfile();
        source.write_all(source_bytes).unwrap();
        let mut target = f.storage.produce_file(metadata(id, b"prefixsuffix")).await.unwrap();
        target.write_all(b"prefix".to_vec()).await.unwrap();
        let token = target.session().unwrap();
        let file = StagedFile {
            history: token.history.clone(),
            id: token.id.clone(),
        };
        let mut capture = f.storage.capture_opened_file(source, target, ByteRange { offset: 0, limit: None }, CaptureConsistency::Stable);
        let result = capture.finish().await;
        if succeeds {
            assert_eq!(result.unwrap().length, source_bytes.len() as u64);
        } else {
            assert!(matches!(result, Err(Error::InvalidInput { .. })));
            assert_eq!(f.storage.file_progress(&file).await.unwrap().length, 6);
            assert_eq!(capture.acknowledged(), 6);
            assert!(capture.finish().await.is_err());
        }
        drop(capture);
    }
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_synchronizes_one_original_production() {
    use std::io::Write;
    let f = Fixture::new(config()).await;
    let (_source_owner, mut source) = shareable_tempfile();
    source.write_all(b"prefix").unwrap();
    let mut definition = metadata("live-opened", b"");
    definition.expected_length = None;
    definition.expected_sha256 = None;
    let target = f.storage.produce_file(definition).await.unwrap();
    let token = target.session().unwrap();
    let staged = StagedFile {
        history: token.history.clone(),
        id: token.id.clone(),
    };
    let session = f.storage.clone();
    let mut capture =
        session.capture_opened_file(source.try_clone().unwrap(), target, ByteRange { offset: 0, limit: None }, CaptureConsistency::Stable);

    assert!(capture.synchronize(6).await.unwrap());
    assert_eq!(capture.acknowledged(), 6);
    assert!(capture.synchronize(6).await.unwrap());
    assert_eq!(capture.acknowledged(), 6);
    let progress = session.file_progress(&staged).await.unwrap();
    assert_eq!(progress.phase, FilePhase::Producing);
    assert_eq!(progress.length, 6);
    source.write_all(b"suffix").unwrap();
    assert!(capture.synchronize(12).await.unwrap());
    assert_eq!(capture.acknowledged(), 12);
    assert_eq!(session.file_progress(&staged).await.unwrap().phase, FilePhase::Producing);
    let reference = capture.finish_at(12).await.unwrap();
    assert_eq!(reference.metadata.id, staged.id);
    assert_eq!(reference.length, 12);
    assert_eq!(reference.sha256, <[u8; 32]>::from(Sha256::digest(b"prefixsuffix")));
    assert_eq!(capture.finish_at(12).await.unwrap(), reference);
    assert_eq!(f.data.control.chunk_offsets.lock().unwrap().as_slice(), &[0, 6]);
    drop((capture, session));
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_finish_rechecks_stable_source_on_repeat() {
    use std::io::{Seek, SeekFrom, Write};
    let f = Fixture::new(config()).await;
    let (_source_owner, mut source) = shareable_tempfile();
    source.write_all(b"original").unwrap();
    let stamp = source.metadata().unwrap().modified().unwrap();
    let target = f.storage.produce_file(metadata("stable-repeat", b"original")).await.unwrap();
    let mut capture = f.storage.capture_opened_file(
        source.try_clone().unwrap(),
        target,
        ByteRange { offset: 0, limit: None },
        CaptureConsistency::Stable,
    );
    let reference = capture.finish().await.unwrap();
    source.seek(SeekFrom::Start(0)).unwrap();
    source.write_all(b"modified").unwrap();
    source.set_modified(std::time::SystemTime::UNIX_EPOCH).unwrap();
    assert!(capture.finish().await.is_err());
    source.seek(SeekFrom::Start(0)).unwrap();
    source.write_all(b"original").unwrap();
    source.set_modified(stamp).unwrap();
    assert_eq!(capture.finish().await.unwrap(), reference);
    drop(capture);
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_discards_unstable_unaccepted_suffix() {
    use std::io::Write;
    let f = Fixture::new(config()).await;
    let (_source_owner, mut source) = shareable_tempfile();
    source.write_all(b"prefixsuffix").unwrap();
    let mut definition = metadata("unstable-opened", b"");
    definition.expected_length = None;
    definition.expected_sha256 = None;
    let mut target = f.storage.produce_file(definition).await.unwrap();
    target.write_all(b"prefix".to_vec()).await.unwrap();
    let mut capture = f.storage.capture_opened_file(
        source.try_clone().unwrap(),
        target,
        ByteRange { offset: 0, limit: None },
        CaptureConsistency::Stable,
    );
    f.data.control.block_read.store(true, Ordering::SeqCst);
    {
        let synchronize = capture.synchronize(12);
        tokio::pin!(synchronize);
        tokio::select! {
            biased;
            _ = &mut synchronize => panic!("prefix verification must block"),
            _ = f.data.control.read_entered.notified() => {}
        }
        source.write_all(b"later").unwrap();
        f.data.control.block_read.store(false, Ordering::SeqCst);
        f.data.control.read_release.notify_waiters();
        assert!(!synchronize.await.unwrap());
    }
    assert_eq!(capture.acknowledged(), 6);
    assert!(capture.synchronize(17).await.unwrap());
    let reference = capture.finish_at(17).await.unwrap();
    assert_eq!(reference.sha256, <[u8; 32]>::from(Sha256::digest(b"prefixsuffixlater")));
    drop(capture);
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_interrupts_only_accepted_cancelled_step() {
    use std::io::Write;
    let mut policy = config();
    policy.response.batch_bytes = NonZeroUsize::new(31).unwrap();
    let f = Fixture::new(policy).await;
    let bytes: Vec<_> = (0..96).map(|value| value as u8).collect();
    let (_source_owner, mut source) = shareable_tempfile();
    source.write_all(&bytes).unwrap();
    let mut definition = metadata("interrupted-opened", b"");
    definition.expected_length = None;
    definition.expected_sha256 = None;
    let target = f.storage.produce_file(definition).await.unwrap();
    let mut capture = f.storage.capture_opened_file(
        source.try_clone().unwrap(),
        target,
        ByteRange { offset: 0, limit: None },
        CaptureConsistency::Stable,
    );
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let synchronize = capture.synchronize(bytes.len() as u64);
        tokio::pin!(synchronize);
        tokio::select! {
            biased;
            _ = &mut synchronize => panic!("accepted append must block"),
            _ = f.data.control.chunk_entered.notified() => {}
        }
    }
    let accepted = f.data.control.chunk_sizes.lock().unwrap()[0];
    assert!(accepted < bytes.len());
    assert_eq!(capture.acknowledged(), 0);
    source.set_len(0).unwrap();
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_waiters();
    let reference = capture.interrupt().await.unwrap();
    assert_eq!(reference.length, accepted as u64);
    assert_eq!(capture.acknowledged(), accepted as u64);
    assert_eq!(reference.sha256, <[u8; 32]>::from(Sha256::digest(&bytes[..accepted])));
    assert_eq!(f.data.control.chunk_offsets.lock().unwrap().as_slice(), &[0]);
    drop(capture);
    f.finish().await;
}

#[tokio::test]
async fn opened_capture_prefix_conflict_survives_source_repair() {
    use std::io::{Seek, SeekFrom, Write};
    let f = Fixture::new(config()).await;
    for (id, truncate) in [("rewrite-opened", false), ("truncate-opened", true)] {
        let (_source_owner, mut source) = shareable_tempfile();
        source.write_all(b"prefix").unwrap();
        let mut definition = metadata(id, b"");
        definition.expected_length = None;
        definition.expected_sha256 = None;
        let target = f.storage.produce_file(definition).await.unwrap();
        let mut capture = f.storage.capture_opened_file(
            source.try_clone().unwrap(),
            target,
            ByteRange { offset: 0, limit: None },
            CaptureConsistency::Stable,
        );
        assert!(capture.synchronize(6).await.unwrap());
        if truncate {
            source.set_len(2).unwrap();
        } else {
            source.seek(SeekFrom::Start(0)).unwrap();
            source.write_all(b"change").unwrap();
            source.set_modified(std::time::SystemTime::UNIX_EPOCH).unwrap();
        }
        assert!(capture.synchronize(if truncate { 2 } else { 6 }).await.is_err());
        source.seek(SeekFrom::Start(0)).unwrap();
        source.write_all(b"prefix").unwrap();
        source.set_len(6).unwrap();
        assert!(capture.synchronize(6).await.is_err());
        let reference = capture.interrupt().await.unwrap();
        assert_eq!(reference.length, 6);
        assert_eq!(reference.sha256, <[u8; 32]>::from(Sha256::digest(b"prefix")));
        drop(capture);
    }
    f.finish().await;
}

#[tokio::test]
async fn opened_clones_keep_independent_read_offsets() {
    use std::io::{Seek, SeekFrom, Write};
    let f = Fixture::new(config()).await;
    let expected: Vec<u8> = (0..256_000).map(|index| (index % 251) as u8).collect();
    let (_source_owner, mut source) = shareable_tempfile();
    source.write_all(&expected).unwrap();
    let mut observer = source.try_clone().unwrap();
    observer.seek(SeekFrom::Start(11)).unwrap();
    let session = f.storage.clone();
    let mut left = session.read_opened_file(source.try_clone().unwrap(), ByteRange { offset: 0, limit: Some(100_000) }).await.unwrap();
    let mut right = session.read_opened_file(source, ByteRange { offset: 128_000, limit: Some(100_000) }).await.unwrap();
    loop {
        let (first, second) = tokio::join!(left.next(), right.next());
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(observer.stream_position().unwrap(), 11);
        let (Some(first), Some(second)) = (first, second) else {
            break;
        };
        let left_offset = first.offset() as usize;
        let right_offset = second.offset() as usize;
        assert_eq!(first.bytes(), &expected[left_offset..left_offset + first.bytes().len()]);
        assert_eq!(second.bytes(), &expected[right_offset..right_offset + second.bytes().len()]);
    }
    assert_eq!(left.completion().unwrap().bytes, 100_000);
    assert_eq!(right.completion().unwrap().bytes, 100_000);
    drop((left, right, session));
    f.finish().await;
}

#[cfg(windows)]
#[tokio::test]
async fn opened_file_rejects_exclusive_handles() {
    let f = Fixture::new(config()).await;
    let session = f.storage.clone();
    let exclusive = tempfile::tempfile().unwrap();
    let result = session.read_opened_file(exclusive, ByteRange { offset: 0, limit: None }).await;
    assert!(matches!(
        result,
        Err(Error::InvalidInput {
            field: "opened file",
            reason: "source handle does not permit an independent read handle"
        })
    ));
    drop(session);
    f.finish().await;
}

#[tokio::test]
async fn frozen_file_keeps_exact_bytes_across_source_mutation_and_capture_retry() {
    use std::io::{Seek, SeekFrom, Write};
    let f = Fixture::new(config()).await;
    let expected = b"independent frozen bytes";
    let (_source_owner, mut source) = shareable_tempfile();
    source.write_all(expected).unwrap();
    let session = f.storage.clone();
    let frozen = session.freeze_opened_file(source.try_clone().unwrap(), ByteRange { offset: 0, limit: None }).await.unwrap();
    assert_eq!(frozen.length(), expected.len() as u64);
    assert_eq!(*frozen.sha256(), <[u8; 32]>::from(Sha256::digest(expected)));
    let clone = frozen.clone();
    source.seek(SeekFrom::Start(0)).unwrap();
    source.write_all(b"mutable source was replaced").unwrap();
    source.set_len(27).unwrap();

    let target = f.storage.produce_file(metadata("frozen-retry", expected)).await.unwrap();
    let mut capture = f.storage.capture_frozen_file(frozen, target).unwrap();
    *f.data.control.permanent_chunk_file.lock().unwrap() = Some("frozen-retry".into());
    assert!(capture.finish().await.is_err());
    *f.data.control.permanent_chunk_file.lock().unwrap() = None;
    let reference = capture.finish().await.unwrap();
    let mut reader = f.storage.read_file(whole(&reference)).await.unwrap();
    assert_eq!(reader.next().await.unwrap().unwrap().bytes(), expected);
    drop((reader, capture));

    let target = f.storage.produce_file(metadata("frozen-cancel", expected)).await.unwrap();
    let mut capture = f.storage.capture_frozen_file(clone, target).unwrap();
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let finish = capture.finish();
        tokio::pin!(finish);
        tokio::select! {
            biased;
            _ = &mut finish => panic!("frozen capture must retain the blocked write"),
            _ = f.data.control.chunk_entered.notified() => {}
        }
    }
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_waiters();
    let reference = capture.finish().await.unwrap();
    assert_eq!(reference.sha256, <[u8; 32]>::from(Sha256::digest(expected)));
    drop((capture, source, session));
    f.finish().await;
}

#[tokio::test]
async fn frozen_file_ranges_include_empty_and_refuse_missing_prefix() {
    use std::io::Write;
    let f = Fixture::new(config()).await;
    let (_source_owner, mut source) = shareable_tempfile();
    source.write_all(b"prefixsuffix").unwrap();
    let session = f.storage.clone();
    for (offset, limit, bytes) in [(0, 0, b"".as_slice()), (0, 6, b"prefix".as_slice()), (6, 6, b"suffix".as_slice())] {
        let frozen = session.freeze_opened_file(source.try_clone().unwrap(), ByteRange { offset, limit: Some(limit) }).await.unwrap();
        assert_eq!(frozen.length(), limit);
        assert_eq!(*frozen.sha256(), <[u8; 32]>::from(Sha256::digest(bytes)));
        let mut reader = frozen.read(ByteRange { offset: 0, limit: None }).await.unwrap();
        drop(frozen);
        let mut actual = Vec::new();
        while let Some(page) = reader.next().await.unwrap() {
            actual.extend_from_slice(page.bytes());
        }
        assert_eq!(actual, bytes);
        assert_eq!(reader.completion().unwrap().bytes, limit);
    }
    assert!(session.freeze_opened_file(source, ByteRange { offset: 0, limit: Some(13) }).await.is_err());
    drop(session);
    f.finish().await;
}

#[tokio::test]
async fn frozen_readers_keep_owners() {
    use std::io::Write;

    let f = Fixture::new(config()).await;
    let (source_owner, mut source) = shareable_tempfile();
    source.write_all(b"prefixsuffix").unwrap();
    let frozen = f.storage.freeze_opened_file(source, ByteRange { offset: 0, limit: None }).await.unwrap();
    drop(source_owner);

    let mut left = frozen.read(ByteRange { offset: 0, limit: Some(6) }).await.unwrap();
    let mut right = frozen.read(ByteRange { offset: 6, limit: Some(6) }).await.unwrap();
    let target = f.storage.produce_file(metadata("frozen-owners", b"prefixsuffix")).await.unwrap();
    let mut capture = f.storage.capture_frozen_file(frozen, target).unwrap();
    let (first, second, reference) = tokio::join!(left.next(), right.next(), capture.finish());
    assert_eq!(first.unwrap().unwrap().bytes(), b"prefix");
    assert_eq!(second.unwrap().unwrap().bytes(), b"suffix");
    assert_eq!(reference.unwrap().length, 12);
    drop(capture);
    assert!(left.next().await.unwrap().is_none());
    assert!(right.next().await.unwrap().is_none());
    drop((left, right));
    f.finish().await;
}

#[tokio::test]
async fn local_errors_keep_context() {
    use harmos_storage::{FileRead, FileSource};

    let f = Fixture::new(config()).await;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing-source");
    let request = FileRead {
        source: FileSource::Local(path.clone()),
        range: ByteRange { offset: 0, limit: None },
    };
    let error = f.storage.read_file(request.clone()).await.err().expect("missing path must fail");
    let Error::Backend { operation, kind, source } = error else { panic!("local error must preserve its backend classification") };
    assert_eq!(operation, "local filesystem");
    assert_eq!(kind, harmos_storage::BackendFailure::Permanent);
    let message = source.to_string();
    assert!(message.contains("inspect path component"));
    assert!(message.contains(path.to_str().unwrap()));
    let native = source.source().unwrap().downcast_ref::<std::io::Error>().unwrap();
    assert_eq!(native.kind(), std::io::ErrorKind::NotFound);
    assert!(native.raw_os_error().is_some());

    std::fs::write(&path, b"payload").unwrap();
    let opened = std::fs::File::open(&path).unwrap();
    let mut local = f.storage.read_file(request).await.unwrap();
    let mut opened = f.storage.read_opened_file(opened, ByteRange { offset: 0, limit: None }).await.unwrap();
    std::fs::File::create(&path).unwrap();
    for reader in [&mut local, &mut opened] {
        let error = reader.next().await.expect_err("truncated source must fail");
        let Error::Backend { source, .. } = error else { panic!("read failure must retain its native source") };
        let message = source.to_string();
        assert!(message.contains("read source"));
        assert!(message.contains(path.to_str().unwrap()));
        let native = source.source().unwrap().downcast_ref::<std::io::Error>().unwrap();
        assert_eq!(native.kind(), std::io::ErrorKind::UnexpectedEof);
    }
    drop((local, opened));
    f.finish().await;
}
