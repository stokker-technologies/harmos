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
use std::num::NonZeroUsize;
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Fixture, config, metadata};
use harmos_runtime::Identifier;
use harmos_runtime::storage::operations::{AbandonFile, AppendFile, CreateFile, FinishFile};
use harmos_runtime::storage::{FilePhase, MutationId, StagedFile};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
async fn fixture() -> Fixture {
    let f = Fixture::new(config()).await;
    harmos_runtime::storage::operations::register(
        &f.runtime.operations,
        "storage",
        crate::operations::Lanes::Open,
        Arc::new(f.storage.clone()),
    )
    .unwrap();
    f
}
fn key(f: &Fixture, key: &str) -> MutationId {
    MutationId {
        window: f.storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("file_preserve_receipts", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let create = CreateFile {
                        mutation: key(&f, "create"),
                        metadata: metadata("shared"),
                    };
                    let producer = f.runtime.operations.call(create.clone()).await.unwrap();
                    assert_eq!(
                        *f.runtime.operations.call(create.clone()).await.unwrap(),
                        *producer
                    );
                    let first = AppendFile {
                        mutation: key(&f, "first"),
                        session: producer.session.clone(),
                        offset: 0,
                        bytes: b"one".to_vec(),
                    };
                    let appended = f.runtime.operations.call(first.clone()).await.unwrap();
                    assert_eq!(appended.length, 3);
                    let second = AppendFile {
                        mutation: key(&f, "second"),
                        offset: 3,
                        bytes: b"two".to_vec(),
                        ..first.clone()
                    };
                    assert_eq!(f.runtime.operations.call(second).await.unwrap().length, 6);
                    assert_eq!(
                        *f.runtime.operations.call(first.clone()).await.unwrap(),
                        *appended
                    );
                    let conflict = AppendFile {
                        bytes: b"bad".to_vec(),
                        ..first.clone()
                    };
                    assert!(f.runtime.operations.call(conflict).await.is_err());
                    let wrong_offset = AppendFile {
                        mutation: key(&f, "wrong-offset"),
                        ..first
                    };
                    assert!(f.runtime.operations.call(wrong_offset).await.is_err());
                    let finish = FinishFile {
                        mutation: key(&f, "finish"),
                        session: producer.session.clone(),
                    };
                    let reference = f.runtime.operations.call(finish.clone()).await.unwrap();
                    assert_eq!(reference.length, 6);
                    assert_eq!(
                        *f.runtime.operations.call(finish.clone()).await.unwrap(),
                        *reference
                    );
                    assert_eq!(*f.runtime.operations.call(create).await.unwrap(), *producer);
                    f.storage.flush().await.unwrap();
                    let restored = super::recovery::restart(&f).await;
                    assert_eq!(
                        restored
                            .storage
                            .finish_file_once(&finish.mutation, &finish.session)
                            .await
                            .unwrap(),
                        *reference
                    );
                    let identity = StagedFile {
                        history: producer.session.history.clone(),
                        id: producer.session.id.clone(),
                    };
                    let mut reader = restored.storage.follow_file(&identity, 0).await.unwrap();
                    assert_eq!(
                        reader
                            .next_batch(NonZeroUsize::new(16).unwrap())
                            .await
                            .unwrap()
                            .unwrap()
                            .bytes,
                        b"onetwo"
                    );
                    assert!(
                        reader
                            .next_batch(NonZeroUsize::MIN)
                            .await
                            .unwrap()
                            .is_none()
                    );
                    drop(reader);
                    restored.finish().await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("creation_append_abandon", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    f.data.control.create_failures.store(3, Ordering::SeqCst);
                    let producer = f
                        .runtime
                        .operations
                        .call(CreateFile {
                            mutation: key(&f, "create"),
                            metadata: metadata("lost"),
                        })
                        .await
                        .unwrap();
                    assert_eq!(producer.session.generation, 1);
                    assert_eq!(f.data.control.create_failures.load(Ordering::SeqCst), 0);
                    f.data.control.chunk_failures.store(3, Ordering::SeqCst);
                    let append = AppendFile {
                        mutation: key(&f, "append"),
                        session: producer.session.clone(),
                        offset: 0,
                        bytes: b"partial".to_vec(),
                    };
                    assert_eq!(
                        f.runtime
                            .operations
                            .call(append.clone())
                            .await
                            .unwrap()
                            .length,
                        7
                    );
                    assert_eq!(
                        *f.data.control.chunk_offsets.lock().unwrap(),
                        vec![0, 0, 0, 0]
                    );
                    f.data.control.abandon_failures.store(3, Ordering::SeqCst);
                    let abandon = AbandonFile {
                        mutation: key(&f, "abandon"),
                        session: producer.session.clone(),
                    };
                    let abandoned = f.runtime.operations.call(abandon.clone()).await.unwrap();
                    assert_eq!(abandoned.phase, FilePhase::Abandoned);
                    assert_eq!(abandoned.length, 7);
                    assert_eq!(
                        *f.runtime.operations.call(abandon).await.unwrap(),
                        *abandoned
                    );
                    assert_eq!(f.runtime.operations.call(append).await.unwrap().length, 7);
                    assert_eq!(f.data.staged.lock().unwrap()["lost"].1, b"partial");
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("append_replays_chunk", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    tokio::time::timeout(Duration::from_secs(5), async {
        let f = fixture().await;
        let producer = f.storage.create_file_once(&key(&f, "create"), &metadata("cancelled")).await.unwrap();
        let id = key(&f, "append");
        f.data.control.block_chunk.store(true, Ordering::SeqCst);
        {
            let append = f.storage.append_file_once(&id, &producer.session, 0, b"bytes");
            tokio::pin!(append);
            tokio::select! { biased; _ = &mut append => panic!("chunk must block"), _ = f.data.control.chunk_entered.notified() => {} }
        }
        f.data.control.block_chunk.store(false, Ordering::SeqCst);
        f.data.control.chunk_release.notify_one();
        let appended = f.storage.append_file_once(&id, &producer.session, 0, b"bytes").await.unwrap();
        assert_eq!(appended.length, 5);
        assert_eq!(*f.data.control.chunk_offsets.lock().unwrap(), vec![0]);
        f.finish().await;
    }).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("appends_claim_exclusively", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let producer = f
                        .storage
                        .create_file_once(&key(&f, "create"), &metadata("offset"))
                        .await
                        .unwrap();
                    let first = key(&f, "first");
                    let second = key(&f, "second");
                    let (first, second) = tokio::join!(
                        f.storage
                            .append_file_once(&first, &producer.session, 0, b"first"),
                        f.storage
                            .append_file_once(&second, &producer.session, 0, b"second")
                    );
                    assert_eq!(first.unwrap().length, 5);
                    assert!(second.is_err());
                    assert_eq!(f.data.staged.lock().unwrap()["offset"].1, b"first");
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("integrity_retains_bytes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let mut metadata = metadata("integrity");
                    metadata.expected_length = Some(6);
                    let producer = f
                        .storage
                        .create_file_once(&key(&f, "create"), &metadata)
                        .await
                        .unwrap();
                    f.storage
                        .append_file_once(&key(&f, "append"), &producer.session, 0, b"bad")
                        .await
                        .unwrap();
                    let id = key(&f, "finish");
                    assert!(
                        f.storage
                            .finish_file_once(&id, &producer.session)
                            .await
                            .is_err()
                    );
                    assert!(
                        f.storage
                            .finish_file_once(&id, &producer.session)
                            .await
                            .is_err()
                    );
                    let identity = StagedFile {
                        history: producer.session.history.clone(),
                        id: producer.session.id.clone(),
                    };
                    assert_eq!(
                        f.storage.file_progress(&identity).await.unwrap().phase,
                        FilePhase::Abandoned
                    );
                    assert_eq!(f.data.staged.lock().unwrap()["integrity"].1, b"bad");
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("save_never_persist", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let through = f.storage.cursor_accepted();
                    let producer = f
                        .storage
                        .create_file_once(&key(&f, "create"), &metadata("frontier"))
                        .await
                        .unwrap();
                    let id = key(&f, "finish");
                    let reference = f
                        .storage
                        .finish_file_once(&id, &producer.session)
                        .await
                        .unwrap();
                    assert!(reference.position.sequence > through.sequence);
                    f.storage.flush_until(&through).await.unwrap();
                    assert!(!f.data.mutation_receipts.lock().unwrap().contains_key(&id));
                    f.storage.flush().await.unwrap();
                    assert!(f.data.mutation_receipts.lock().unwrap().contains_key(&id));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("bytes_survive_restart", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let first = f
                        .storage
                        .write_file(metadata("first-finished"), b"first")
                        .await
                        .unwrap();
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    let recovered = super::recovery::restart(&f).await;
                    let second = recovered
                        .storage
                        .write_file(metadata("second-finished"), b"second")
                        .await
                        .unwrap();
                    assert_eq!(first.position, second.position);
                    for (reference, expected) in
                        [(first, b"first".as_slice()), (second, b"second".as_slice())]
                    {
                        let identity = StagedFile {
                            history: reference.position.history,
                            id: reference.metadata.id,
                        };
                        let mut reader = recovered.storage.follow_file(&identity, 0).await.unwrap();
                        let chunk = reader
                            .next_batch(NonZeroUsize::new(32).unwrap())
                            .await
                            .unwrap()
                            .unwrap();
                        assert_eq!(chunk.bytes, expected);
                        assert!(
                            reader
                                .next_batch(NonZeroUsize::MIN)
                                .await
                                .unwrap()
                                .is_none()
                        );
                    }
                    recovered.finish().await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("finish_reconciles_lost", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = fixture().await;
                    let producer = f
                        .storage
                        .create_file_once(&key(&f, "create"), &metadata("lost-finish"))
                        .await
                        .unwrap();
                    f.storage
                        .append_file_once(&key(&f, "append"), &producer.session, 0, b"finished")
                        .await
                        .unwrap();
                    f.data.control.finish_failures.store(1, Ordering::SeqCst);
                    let request = FinishFile {
                        mutation: key(&f, "finish"),
                        session: producer.session,
                    };
                    let result = f.runtime.operations.call(request.clone()).await.unwrap();
                    let replay = f.runtime.operations.call(request).await.unwrap();
                    assert_eq!(result, replay);
                    assert_eq!(
                        f.data.control.finish_positions.lock().unwrap().as_slice(),
                        &[result.position.sequence, result.position.sequence]
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("retries_never_multiply", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    harmos_runtime::storage::operations::register(
                        &f.runtime.operations,
                        "storage",
                        crate::operations::Lanes::Open,
                        Arc::new(f.storage.clone()),
                    )
                    .unwrap();
                    let request = CreateFile {
                        mutation: key(&f, "create"),
                        metadata: metadata("retry-budget"),
                    };
                    f.data.control.create_failures.store(5, Ordering::SeqCst);
                    assert!(f.runtime.operations.call(request.clone()).await.is_err());
                    assert_eq!(f.data.control.create_failures.load(Ordering::SeqCst), 1);
                    f.data.control.create_failures.store(0, Ordering::SeqCst);
                    let producer = f.runtime.operations.call(request).await.unwrap();
                    f.storage
                        .abandon_file_once(&key(&f, "abandon"), &producer.session)
                        .await
                        .unwrap();
                    f.finish().await;
                });
        });
    }
}
