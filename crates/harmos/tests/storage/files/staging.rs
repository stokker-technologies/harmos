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

#[path = "../staging/mutations.rs"]
mod mutations;
#[path = "../staging/production.rs"]
mod production;
#[path = "../staging/recovery.rs"]
mod recovery;

/* === Re-Exports === */

/* === Imports === */
use crate::backend::storage_settings;
// Standard Library Imports
use std::sync::atomic::Ordering;
use std::time::Duration;
// Third Party Imports
// Local Imports
use super::{Fixture, config};
use harmos::Identifier;
use harmos::storage::FileMetadata;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn metadata(name: &str) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(name).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    }
}

/* === Tests === */

fn own_contract() {
    {
        scenario_runtime::check("resume_preserves_offset", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut first = f.storage.produce_file(metadata("resume")).await.unwrap();
                    first.write_all(b"first").await.unwrap();
                    let token = first.session().unwrap().clone();
                    let mut resumed = f.storage.resume_file(&token).await.unwrap();
                    assert_eq!(resumed.length(), 5);
                    assert_eq!(resumed.session().unwrap().generation, token.generation + 1);
                    assert!(first.write_all(b"stale").await.is_err());
                    assert!(first.finish().await.is_err());
                    assert!(first.abort().await.is_err());
                    drop(first);
                    resumed.write_all(b"second").await.unwrap();
                    assert_eq!(resumed.length(), 11);
                    let token = resumed.session().unwrap().clone();
                    drop(resumed);
                    assert!(f.data.abandoned_files.lock().unwrap().is_empty());
                    let mut resumed = f.storage.resume_file(&token).await.unwrap();
                    assert_eq!(resumed.length(), 11);
                    let result = resumed.finish().await.unwrap();
                    assert_eq!(result.length, 11);
                    assert_eq!(
                        f.data.blobs.lock().unwrap()[result.metadata.id.as_str()].1,
                        b"firstsecond"
                    );
                    assert!(f.storage.resume_file(&token).await.is_err());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reconciled_resume_returns", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let f = Fixture::new(config()).await;
                        let mut file = f.storage.produce_file(metadata("cancelled")).await.unwrap();
                        let token = file.session().unwrap().clone();
                        f.data.control.block_chunk.store(true, Ordering::SeqCst);
                        {
                            let write = file.write_all(b"retained");
                            tokio::pin!(write);
                            tokio::select! {
                                biased;
                                _ = &mut write => panic!("expected blocked physical write"),
                                _ = f.data.control.chunk_entered.notified() => {}
                            }
                        }
                        assert!(file.write_all(b"ambiguous").await.is_err());
                        drop(file);
                        let mut resumed = {
                            let resume = f.storage.resume_file(&token);
                            tokio::pin!(resume);
                            std::future::poll_fn(|cx| {
                                assert!(
                                    std::future::Future::poll(resume.as_mut(), cx).is_pending()
                                );
                                std::task::Poll::Ready(())
                            })
                            .await;
                            f.data.control.block_chunk.store(false, Ordering::SeqCst);
                            f.data.control.chunk_release.notify_one();
                            resume.await.unwrap()
                        };
                        assert_eq!(resumed.length(), 8);
                        resumed.write_all(b" bytes").await.unwrap();
                        let reference = resumed.finish().await.unwrap();
                        assert_eq!(
                            f.data.blobs.lock().unwrap()[reference.metadata.id.as_str()].1,
                            b"retained bytes"
                        );
                        f.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("seconds_retains_bytes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut file = f.storage.produce_file(metadata("expired")).await.unwrap();
                    file.write_all(b"partial").await.unwrap();
                    let token = file.session().unwrap().clone();
                    tokio::time::advance(Duration::from_secs(61)).await;
                    for _ in 0..10 {
                        tokio::task::yield_now().await;
                    }
                    assert!(f.data.abandoned_files.lock().unwrap().is_empty());
                    file.write_all(b" after idle").await.unwrap();
                    assert_eq!(file.session().unwrap(), &token);
                    drop(file);
                    tokio::time::advance(Duration::from_secs(59)).await;
                    assert!(f.data.abandoned_files.lock().unwrap().is_empty());
                    tokio::time::advance(Duration::from_secs(1)).await;
                    for _ in 0..10 {
                        if f.data.abandoned_files.lock().unwrap().contains("expired") {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                    assert!(f.data.abandoned_files.lock().unwrap().contains("expired"));
                    assert_eq!(
                        f.data.staged.lock().unwrap()["expired"].1,
                        b"partial after idle"
                    );
                    assert!(f.storage.resume_file(&token).await.is_err());
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    let unadopted = f.storage.produce_file(metadata("unadopted")).await.unwrap();
                    let mut unadopted_token = unadopted.session().unwrap().clone();
                    f.data.control.block_fence.store(true, Ordering::SeqCst);
                    {
                        let resume = f.storage.resume_file(&unadopted_token);
                        tokio::pin!(resume);
                        tokio::select! {
                            biased;
                            _ = &mut resume => panic!("expected blocked resume"),
                            _ = f.data.control.fence_entered.notified() => {}
                        }
                    }
                    f.data.control.block_fence.store(false, Ordering::SeqCst);
                    f.data.control.fence_release.notify_one();
                    for _ in 0..100 {
                        if f.data.file_generations.lock().unwrap().get("unadopted") == Some(&2) {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                    assert_eq!(
                        f.data.file_generations.lock().unwrap().get("unadopted"),
                        Some(&2)
                    );
                    for _ in 0..10 {
                        tokio::task::yield_now().await;
                    }
                    unadopted_token.generation = 2;
                    tokio::time::advance(Duration::from_secs(61)).await;
                    assert!(f.storage.resume_file(&unadopted_token).await.is_err());
                    drop(unadopted);
                    let data = f.data.clone();
                    f.finish().await;
                    assert_eq!(
                        data.staged.lock().unwrap()["expired"].1,
                        b"partial after idle"
                    );
                });
        });
    }
    {
        scenario_runtime::check("abandon_retains_bytes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut file = f.storage.produce_file(metadata("abandoned")).await.unwrap();
                    file.write_all(b"partial").await.unwrap();
                    file.abort().await.unwrap();
                    assert_eq!(f.data.staged.lock().unwrap()["abandoned"].1, b"partial");
                    assert!(f.storage.produce_file(metadata("abandoned")).await.is_err());
                    assert_eq!(f.data.staged.lock().unwrap()["abandoned"].1, b"partial");
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("follow_replays_prefix", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::{FilePhase, StagedFile};
                    use std::num::NonZeroUsize;
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let f = Fixture::new(config()).await;
                        let mut producer =
                            f.storage.produce_file(metadata("growing")).await.unwrap();
                        producer.write_all(b"first").await.unwrap();
                        let token = producer.session().unwrap();
                        let identity = StagedFile {
                            history: token.history.clone(),
                            id: token.id.clone(),
                        };
                        let mut reader = f.storage.follow_file(&identity, 0).await.unwrap();
                        let max = NonZeroUsize::new(3).unwrap();
                        assert_eq!(
                            reader.next_chunk(max).await.unwrap().unwrap().as_bytes(),
                            b"fir"
                        );
                        assert_eq!(
                            reader.next_chunk(max).await.unwrap().unwrap().as_bytes(),
                            b"st"
                        );
                        assert_eq!(reader.offset(), 5);
                        {
                            let next = reader.next_chunk(max);
                            tokio::pin!(next);
                            std::future::poll_fn(|cx| {
                                assert!(std::future::Future::poll(next.as_mut(), cx).is_pending());
                                std::task::Poll::Ready(())
                            })
                            .await;
                            producer.write_all(b"second").await.unwrap();
                            assert_eq!(next.await.unwrap().unwrap().as_bytes(), b"sec");
                        }
                        drop(reader);
                        let mut resumed = f.storage.follow_file(&identity, 8).await.unwrap();
                        assert_eq!(
                            resumed.next_chunk(max).await.unwrap().unwrap().as_bytes(),
                            b"ond"
                        );
                        producer.finish().await.unwrap();
                        assert!(resumed.next_chunk(max).await.unwrap().is_none());
                        assert_eq!(resumed.progress().phase, FilePhase::Finished);
                        assert_eq!(resumed.offset(), 11);
                        assert!(f.storage.follow_file(&identity, 12).await.is_err());
                        drop(resumed);
                        let mut replay = f.storage.follow_file(&identity, 0).await.unwrap();
                        let mut all = Vec::new();
                        while let Some(chunk) = replay.next_chunk(max).await.unwrap() {
                            all.extend_from_slice(&chunk);
                        }
                        assert_eq!(all, b"firstsecond");
                        drop(replay);
                        f.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("files_replay_partial", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::{FilePhase, StagedFile};
                    use std::num::NonZeroUsize;
                    let f = Fixture::new(config()).await;
                    let mut producer = f
                        .storage
                        .produce_file(metadata("partial-reader"))
                        .await
                        .unwrap();
                    producer.write_all(b"partial").await.unwrap();
                    let token = producer.session().unwrap();
                    let identity = StagedFile {
                        history: token.history.clone(),
                        id: token.id.clone(),
                    };
                    let mut reader = f.storage.follow_file(&identity, 0).await.unwrap();
                    producer.abort().await.unwrap();
                    let max = NonZeroUsize::new(100).unwrap();
                    assert_eq!(
                        reader.next_chunk(max).await.unwrap().unwrap().as_bytes(),
                        b"partial"
                    );
                    assert!(reader.next_chunk(max).await.unwrap().is_none());
                    assert_eq!(reader.progress().phase, FilePhase::Abandoned);
                    drop(reader);
                    let mut replay = f.storage.follow_file(&identity, 3).await.unwrap();
                    assert_eq!(
                        replay.next_chunk(max).await.unwrap().unwrap().as_bytes(),
                        b"tial"
                    );
                    assert!(replay.next_chunk(max).await.unwrap().is_none());
                    assert_eq!(replay.progress().phase, FilePhase::Abandoned);
                    drop(replay);
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
                    use harmos::storage::StagedFile;
                    use std::num::NonZeroUsize;
                    let f = Fixture::new(config()).await;
                    let mut producer = f
                        .storage
                        .produce_file(metadata("cancel-reader"))
                        .await
                        .unwrap();
                    producer.write_all(b"first").await.unwrap();
                    let token = producer.session().unwrap();
                    let identity = StagedFile {
                        history: token.history.clone(),
                        id: token.id.clone(),
                    };
                    let mut reader = f.storage.follow_file(&identity, 0).await.unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    {
                        let read = reader.next_chunk(NonZeroUsize::new(3).unwrap());
                        tokio::pin!(read);
                        tokio::select! {
                            biased;
                            _ = &mut read => panic!("expected blocked physical read"),
                            _ = f.data.control.read_entered.notified() => {}
                        }
                    }
                    assert_eq!(reader.offset(), 0);
                    drop(reader);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    producer.write_all(b"second").await.unwrap();
                    assert_eq!(producer.finish().await.unwrap().length, 11);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cannot_expose_private", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::StagedFile;
                    let f = Fixture::new(config()).await;
                    let mut private = f.storage.create_file(metadata("private")).await.unwrap();
                    private.write_all(b"private bytes").await.unwrap();
                    let identity = StagedFile {
                        history: f.storage.cursor_accepted().history,
                        id: Identifier::new("private").unwrap(),
                    };
                    assert!(f.storage.follow_file(&identity, 0).await.is_err());
                    assert!(f.storage.file_progress(&identity).await.is_err());
                    private.abort().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("chunks_retain_read", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::StagedFile;
                    use std::num::NonZeroUsize;
                    let f = Fixture::new(config()).await;
                    let mut producer = f.storage.produce_file(metadata("charged")).await.unwrap();
                    producer.write_all(b"retained payload").await.unwrap();
                    let token = producer.session().unwrap();
                    let identity = StagedFile {
                        history: token.history.clone(),
                        id: token.id.clone(),
                    };
                    let mut reader = f.storage.follow_file(&identity, 0).await.unwrap();
                    let chunk = reader
                        .next_chunk(NonZeroUsize::new(4).unwrap())
                        .await
                        .unwrap()
                        .unwrap();
                    let held = f.storage.status().read_bytes;
                    drop(reader);
                    assert!(f.storage.status().read_bytes < held);
                    assert!(f.storage.status().read_bytes >= chunk.len());
                    drop(chunk);
                    assert_eq!(f.storage.status().read_bytes, 0);
                    producer.abort().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("expired_leases_unchanged", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.file_session_timeout = Duration::from_secs(2);
                    let f = Fixture::new(settings).await;
                    let mut producer = f.storage.produce_file(metadata("late")).await.unwrap();
                    let token = producer.session().unwrap().clone();
                    producer.write_all(b"retained").await.unwrap();
                    drop(producer);
                    tokio::time::advance(Duration::from_secs(2)).await;
                    assert!(f.storage.resume_file(&token).await.is_err());
                    assert_eq!(f.data.staged.lock().unwrap()["late"].1, b"retained");
                    f.finish().await;
                    assert_eq!(
                        storage_settings().file_session_timeout,
                        Duration::from_secs(60)
                    );
                });
        });
    }
    {
        scenario_runtime::check("failure_preserves_retry", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut producer = f
                        .storage
                        .produce_file(metadata("retry-abandon"))
                        .await
                        .unwrap();
                    producer.write_all(b"partial").await.unwrap();
                    f.data.control.fail_abort.store(true, Ordering::SeqCst);
                    assert!(producer.abort().await.is_err());
                    assert_eq!(f.data.staged.lock().unwrap()["retry-abandon"].1, b"partial");
                    f.data.control.fail_abort.store(false, Ordering::SeqCst);
                    producer.abort().await.unwrap();
                    assert!(
                        f.data
                            .abandoned_files
                            .lock()
                            .unwrap()
                            .contains("retry-abandon")
                    );
                    assert_eq!(f.data.staged.lock().unwrap()["retry-abandon"].1, b"partial");
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("admitted_finish_reconciles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut producer = f
                        .storage
                        .produce_file(metadata("retry-finish"))
                        .await
                        .unwrap();
                    producer.write_all(b"complete").await.unwrap();
                    f.data.control.finish_failures.store(1, Ordering::SeqCst);
                    assert!(producer.finish().await.is_err());
                    tokio::time::advance(Duration::from_secs(61)).await;
                    let reference = producer.finish().await.unwrap();
                    assert_eq!(reference.length, 8);
                    assert_eq!(f.data.blobs.lock().unwrap().len(), 1);
                    assert!(f.data.abandoned_files.lock().unwrap().is_empty());
                    f.finish().await;
                });
        });
    }
}

#[test]
fn staged_files() {
    own_contract();
    mutations::contract_scenario();
    production::contract_scenario();
    recovery::contract_scenario();
}
