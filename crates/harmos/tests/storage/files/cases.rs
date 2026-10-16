/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::atomic::AtomicUsize;

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

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("file_wait_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.query_execution.timeout = Duration::from_secs(10);
                    let f = Fixture::new(settings).await;
                    let reference = f
                        .storage
                        .write_file(metadata("owned-read", b"original"), b"original")
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let mut reader = view.read_file(&reference).await.unwrap();
                    let (started, entered) = tokio::sync::oneshot::channel();
                    let (release, waiting) = tokio::sync::oneshot::channel();
                    *f.data.control.accepted_file_read.lock().unwrap() = Some((started, waiting));
                    let observer = tokio::spawn(async move {
                        reader.next_chunk(NonZeroUsize::new(8).unwrap()).await
                    });
                    entered.await.unwrap();
                    observer.abort();
                    assert!(observer.await.unwrap_err().is_cancelled());
                    assert!(matches!(
                        view.begin_computation(),
                        Err(Error::NotReady { .. })
                    ));
                    let releases = f.data.control.releases.load(Ordering::SeqCst);
                    tokio::time::advance(Duration::from_secs(11)).await;
                    assert_eq!(f.data.control.releases.load(Ordering::SeqCst), releases);
                    assert!(matches!(
                        view.begin_computation(),
                        Err(Error::NotReady { .. })
                    ));
                    release.send(()).unwrap();
                    for _ in 0..100 {
                        tokio::task::yield_now().await;
                        if matches!(
                            view.begin_computation(),
                            Err(Error::ResourceExhausted { .. })
                        ) {
                            drop(view);
                            f.finish().await;
                            return;
                        }
                    }
                    panic!("physical result did not settle its original computation");
                });
        });
    }
    {
        scenario_runtime::check("streams_flush_controls", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
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
                    assert_eq!(f.storage.status().saved.sequence, 0);
                    let mut durable = f
                        .storage
                        .query(file_query(ReadLimits {
                            rows: NonZeroUsize::new(16).unwrap(),
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        }))
                        .await
                        .unwrap();
                    assert!(durable.next_page().await.unwrap().is_none());
                    let mut reader = f.storage.read_file(&reference).await.unwrap();
                    let mut actual = Vec::new();
                    while let Some(chunk) = reader
                        .next_chunk(NonZeroUsize::new(47).unwrap())
                        .await
                        .unwrap()
                    {
                        actual.extend_from_slice(chunk.as_bytes());
                    }
                    assert_eq!(actual, bytes);
                    f.storage.flush().await.unwrap();
                    assert_eq!(
                        f.storage.status().saved.sequence,
                        reference.position.sequence
                    );
                    drop(durable);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("chunk_retries_offset", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_retries = 1;
                    let f = Fixture::new(policy).await;
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
                    let mut reader = f.storage.read_file(&reference).await.unwrap();
                    assert_eq!(
                        reader
                            .next_chunk(NonZeroUsize::new(100).unwrap())
                            .await
                            .unwrap()
                            .unwrap()
                            .as_bytes(),
                        bytes
                    );
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("uncertain_finish_reuses", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let bytes = b"finished once";
                    let mut file = f
                        .storage
                        .create_file(metadata("finish", bytes))
                        .await
                        .unwrap();
                    file.write_all(bytes).await.unwrap();
                    f.data.control.finish_failures.store(1, Ordering::SeqCst);
                    assert!(file.finish().await.is_err());
                    assert_eq!(f.storage.status().accepted.sequence, 0);
                    let reference = file.finish().await.unwrap();
                    assert_eq!(reference.position.sequence, 1);
                    assert_eq!(
                        f.data.control.finish_positions.lock().unwrap().as_slice(),
                        &[1, 1]
                    );
                    assert_eq!(f.data.blobs.lock().unwrap().len(), 1);
                    assert!(f.data.files.lock().unwrap().is_empty());
                    file.abort().await.unwrap();
                    assert_eq!(f.data.blobs.lock().unwrap().len(), 1);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("create_keeps_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
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
                });
        });
    }
    {
        scenario_runtime::check("abort_failure_retryable", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
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
                });
        });
    }
    {
        scenario_runtime::check("failure_never_publishes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
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
                });
        });
    }
    {
        scenario_runtime::check("chunk_never_advance", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let bytes = b"immutable bytes";
                    let reference = f
                        .storage
                        .write_file(metadata("read", bytes), bytes)
                        .await
                        .unwrap();
                    let mut reader = f.storage.read_file(&reference).await.unwrap();
                    f.data.control.corrupt_file.store(true, Ordering::SeqCst);
                    assert!(
                        reader
                            .next_chunk(NonZeroUsize::new(100).unwrap())
                            .await
                            .is_err()
                    );
                    assert_eq!(reader.offset(), 0);
                    f.data.control.corrupt_file.store(false, Ordering::SeqCst);
                    assert_eq!(
                        reader
                            .next_chunk(NonZeroUsize::new(100).unwrap())
                            .await
                            .unwrap()
                            .unwrap()
                            .as_bytes(),
                        bytes
                    );
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancelled_chunk_aborts", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
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
                });
        });
    }
    {
        scenario_runtime::check("cancelled_finish_drained", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
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
                });
        });
    }
    {
        scenario_runtime::check("read_preserves_offset", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let bytes = b"read retry";
                    let reference = f
                        .storage
                        .write_file(metadata("reader", bytes), bytes)
                        .await
                        .unwrap();
                    let mut reader = f.storage.read_file(&reference).await.unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    {
                        let read = reader.next_chunk(NonZeroUsize::new(100).unwrap());
                        tokio::pin!(read);
                        tokio::select! {
                            biased;
                            _ = &mut read => panic!("read must block"),
                            _ = f.data.control.read_entered.notified() => {}
                        }
                    }
                    assert_eq!(reader.offset(), 0);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    assert_eq!(
                        reader
                            .next_chunk(NonZeroUsize::new(100).unwrap())
                            .await
                            .unwrap()
                            .unwrap()
                            .as_bytes(),
                        bytes
                    );
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reads_survive_save", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let bytes = b"captured";
                    let first = f
                        .storage
                        .write_file(metadata("first", bytes), bytes)
                        .await
                        .unwrap();
                    let mut file = f.storage.read_file(&first).await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let later = f
                        .storage
                        .write_file(metadata("later", b"later"), b"later")
                        .await
                        .unwrap();
                    let limits = ReadLimits {
                        rows: NonZeroUsize::new(1).unwrap(),
                        bytes: NonZeroUsize::new(4096).unwrap(),
                    };
                    let mut reader = f.storage.query_in(&view, file_query(limits)).unwrap();
                    assert!(reader.next_page().await.unwrap().is_none());
                    assert!(view.read_file(&first).await.is_err());
                    assert!(view.read_file(&later).await.is_err());
                    let mut saved = f.storage.query(file_query(limits)).await.unwrap();
                    assert_eq!(&*saved.next_page().await.unwrap().unwrap(), &[first]);
                    assert!(saved.next_page().await.unwrap().is_none());
                    assert_eq!(
                        file.next_chunk(NonZeroUsize::new(100).unwrap())
                            .await
                            .unwrap()
                            .unwrap()
                            .as_bytes(),
                        bytes
                    );
                    drop(file);
                    drop(reader);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("tiny_budget_streams", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.request.max_pending_write_bytes = NonZeroUsize::new(20_000).unwrap();
                    policy.target_batch_bytes = policy.request.max_pending_write_bytes;
                    policy.max_batch_bytes = policy.request.max_pending_write_bytes;
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
                });
        });
    }
    {
        scenario_runtime::check("admission_flushes_queues", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    tokio::time::timeout(Duration::from_secs(5), async {
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
                    })
                    .await
                    .expect("file stages must not wait for the periodic flush");
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("file_chunks_renew", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.view_idle_timeout = Duration::from_millis(10);
                    let f = Fixture::new(policy).await;
                    let file = f
                        .storage
                        .write_file(metadata("idle", b"abcd"), b"abcd")
                        .await
                        .unwrap();
                    let mut reader = f.storage.read_file(&file).await.unwrap();
                    for byte in b"abcd" {
                        tokio::time::advance(Duration::from_millis(6)).await;
                        assert_eq!(
                            reader
                                .next_chunk(NonZeroUsize::new(1).unwrap())
                                .await
                                .unwrap()
                                .unwrap()
                                .as_bytes(),
                            [*byte]
                        );
                    }
                    assert!(
                        reader
                            .next_chunk(NonZeroUsize::new(1).unwrap())
                            .await
                            .unwrap()
                            .is_none()
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reads_reject_forged", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
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
                                4 => {
                                    forged.position.history.epoch =
                                        Identifier::new("other").unwrap()
                                }
                                _ => forged.position.sequence = 0,
                            }
                            assert!(
                                f.storage.read_file(&forged).await.is_err(),
                                "saved={saved}, field={field}"
                            );
                        }
                        let mut reader = f.storage.read_file(&file).await.unwrap();
                        assert_eq!(
                            reader
                                .next_chunk(NonZeroUsize::new(3).unwrap())
                                .await
                                .unwrap()
                                .unwrap()
                                .as_bytes(),
                            b"abc"
                        );
                        assert_eq!(f.storage.status().saved.sequence, u64::from(saved));
                    }
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reader_keeps_metadata", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let file = f
                        .storage
                        .write_file(metadata("owned", b"abc"), b"abc")
                        .await
                        .unwrap();
                    let mut reader = f.storage.read_file(&file).await.unwrap();
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    assert!(f.storage.status().read_bytes > 0);
                    assert_eq!(
                        reader
                            .next_chunk(NonZeroUsize::new(3).unwrap())
                            .await
                            .unwrap()
                            .unwrap()
                            .as_bytes(),
                        b"abc"
                    );
                    tokio::time::timeout(Duration::from_secs(1), async {
                        while f.data.control.releases.load(Ordering::SeqCst) == 0 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    // The exhausted reader still owns the immutable reference exposed by reference().
                    assert_eq!(reader.reference(), &file);
                    assert!(f.storage.status().read_bytes > 0);
                    drop(reader);
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("chunk_retains_charge", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let file = f
                        .storage
                        .write_file(metadata("loan", b"abc"), b"abc")
                        .await
                        .unwrap();
                    let mut reader = f.storage.read_file(&file).await.unwrap();
                    let metadata_bytes = f.storage.status().read_bytes;
                    let chunk = reader
                        .next_chunk(NonZeroUsize::new(3).unwrap())
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(f.storage.status().read_bytes > metadata_bytes);
                    drop(reader);
                    let charged = f.storage.status().read_bytes;
                    assert!(charged >= 3);
                    assert_eq!(chunk.as_bytes(), b"abc");
                    f.storage.flush().await.unwrap();
                    let retained = f.storage.status().read_bytes;
                    assert!(retained >= chunk.len());
                    assert!(retained <= charged);
                    let pointer = chunk.as_bytes().as_ptr();
                    let input = chunk.into_query_value();
                    assert_eq!(input.as_ptr(), pointer);
                    assert_eq!(input.as_slice(), b"abc");
                    input.check_origin(&f.storage).unwrap();
                    assert_eq!(f.storage.status().read_bytes, retained);
                    drop(input);
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projection_retains_input", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let file = f
                        .storage
                        .write_file(metadata("native", b"17"), b"17")
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let mut query = FileQuery::default();
                    query.options.decoded_bytes = Some(NonZeroUsize::new(4096).unwrap());
                    let mut reader = f.storage.query(query).await.unwrap();
                    reader.retain_view().unwrap();
                    let view = reader.view().unwrap().clone();
                    let mut bytes = view.read_file(&file).await.unwrap();
                    let input = bytes
                        .next_chunk(NonZeroUsize::new(2).unwrap())
                        .await
                        .unwrap()
                        .unwrap()
                        .into_query_value();
                    drop(bytes);
                    let page = reader.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0], file);
                    drop(page);
                    assert!(reader.next_page().await.unwrap().is_none());
                    let retained = f.storage.status().read_bytes;
                    let calls = AtomicUsize::new(0);
                    // Output Beyond The Shared Read Budget Is Refused Before The Callback.
                    let oversized =
                        NonZeroUsize::new(config().budget.read_memory_bytes.get() + 1).unwrap();
                    let refused = reader.project(&input, oversized, |_, _| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok(0u64)
                    });
                    assert!(matches!(refused, Err(Error::ResourceExhausted { .. })));
                    assert_eq!(calls.load(Ordering::SeqCst), 0);
                    assert_eq!(f.storage.status().read_bytes, retained);
                    let failure = reader.project(
                        &input,
                        NonZeroUsize::new(64).unwrap(),
                        |_, _| -> harmos::Result<u64> {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Err(Error::InvalidInput {
                                field: "fixture decode",
                                reason: "retry original input",
                            })
                        },
                    );
                    assert!(matches!(
                        failure,
                        Err(Error::InvalidInput {
                            field: "fixture decode",
                            ..
                        })
                    ));
                    assert_eq!(input.as_slice(), b"17");
                    assert_eq!(f.storage.status().read_bytes, retained);
                    let output = reader
                        .project(&input, NonZeroUsize::new(64).unwrap(), |value, _| {
                            calls.fetch_add(1, Ordering::SeqCst);
                            serde_json::from_slice::<u64>(value).map_err(|source| {
                                Error::backend(
                                    "fixture native decode",
                                    BackendFailure::Corrupt,
                                    source,
                                )
                            })
                        })
                        .unwrap();
                    assert_eq!(*output, 17);
                    assert_eq!(calls.load(Ordering::SeqCst), 2);
                    assert_eq!(input.as_slice(), b"17");
                    drop(output);
                    assert_eq!(f.storage.status().read_bytes, retained);
                    drop((input, reader, view));
                    f.finish().await;
                });
        });
    }
}
