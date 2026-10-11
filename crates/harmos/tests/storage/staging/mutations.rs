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
use harmos::storage::operations::{FileMutation, Operations, ResumeFile};
use harmos::storage::{MutationId, StorageManager};
use harmos::{Error, Identifier};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn identity(storage: &StorageManager, key: &str) -> MutationId {
    MutationId {
        window: storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("repeated_resume_returns", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut file = f.storage.produce_file(metadata("once")).await.unwrap();
                    file.write_all(b"prefix").await.unwrap();
                    let token = file.session().unwrap().clone();
                    let id = identity(&f.storage, "resume");
                    let first = f.storage.resume_file_once(&id, &token).await.unwrap();
                    assert_eq!(first.length, 6);
                    assert_eq!(first.session.generation, 2);
                    assert_eq!(
                        f.storage.resume_file_once(&id, &token).await.unwrap(),
                        first
                    );
                    assert!(file.write_all(b"stale").await.is_err());
                    let mut later = f.storage.resume_file(&first.session).await.unwrap();
                    later.write_all(b"tail").await.unwrap();
                    assert_eq!(
                        f.storage.resume_file_once(&id, &token).await.unwrap(),
                        first
                    );
                    assert!(
                        f.storage
                            .resume_file_once(&id, later.session().unwrap())
                            .await
                            .is_err()
                    );
                    assert_eq!(later.session().unwrap().generation, 3);
                    later.abort().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("operation_retries_three", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    Operations::register(
                        &f.runtime.operations,
                        "storage",
                        crate::operations::Lanes::Open,
                        Arc::new(f.storage.clone()),
                    )
                    .unwrap();
                    let file = f.storage.produce_file(metadata("retry")).await.unwrap();
                    let request = FileMutation {
                        mutation: identity(&f.storage, "resume"),
                        session: file.session().unwrap().clone(),
                    };
                    f.data.control.fence_failures.store(3, Ordering::SeqCst);
                    let first = f
                        .runtime
                        .operations
                        .call::<ResumeFile>(request.clone())
                        .await
                        .unwrap();
                    assert_eq!(first.session.generation, 2);
                    assert_eq!(f.data.control.fence_failures.load(Ordering::SeqCst), 0);
                    assert_eq!(
                        *f.runtime
                            .operations
                            .call::<ResumeFile>(request)
                            .await
                            .unwrap(),
                        *first
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancelled_resume_wait", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    tokio::time::timeout(Duration::from_secs(5), async {
        let f = Fixture::new(config()).await;
        let file = f.storage.produce_file(metadata("cancelled")).await.unwrap();
        let token = file.session().unwrap().clone();
        let id = identity(&f.storage, "resume");
        f.data.control.block_fence.store(true, Ordering::SeqCst);
        {
            let first = f.storage.resume_file_once(&id, &token);
            tokio::pin!(first);
            tokio::select! { biased; _ = &mut first => panic!("fence must block"), _ = f.data.control.fence_entered.notified() => {} }
        }
        f.data.control.block_fence.store(false, Ordering::SeqCst);
        f.data.control.fence_release.notify_one();
        let result = f.storage.resume_file_once(&id, &token).await.unwrap();
        assert_eq!(result.session.generation, 2);
        assert_eq!(f.storage.resume_file_once(&id, &token).await.unwrap(), result);
        f.finish().await;
    }).await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("identity_never_becomes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.mutation_retention = Duration::from_secs(1);
                    let f = Fixture::new(settings).await;
                    let file = f.storage.produce_file(metadata("expiry")).await.unwrap();
                    let token = file.session().unwrap().clone();
                    let id = identity(&f.storage, "resume");
                    let result = f.storage.resume_file_once(&id, &token).await.unwrap();
                    tokio::time::advance(Duration::from_secs(1)).await;
                    let next = identity(&f.storage, "resume");
                    assert_ne!(id.window, next.window);
                    assert!(f.storage.resume_file_once(&id, &token).await.is_err());
                    assert_eq!(
                        f.storage
                            .resume_file_once(&next, &result.session)
                            .await
                            .unwrap()
                            .session
                            .generation,
                        3
                    );
                    assert!(f.storage.resume_file_once(&id, &token).await.is_err());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("capacity_refuses_work", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.max_mutations = NonZeroUsize::MIN;
                    let f = Fixture::new(settings).await;
                    let file = f.storage.produce_file(metadata("bounded")).await.unwrap();
                    let token = file.session().unwrap().clone();
                    let first_id = identity(&f.storage, "first");
                    let first = f.storage.resume_file_once(&first_id, &token).await.unwrap();
                    let second_id = identity(&f.storage, "second");
                    assert!(matches!(
                        f.storage.resume_file_once(&second_id, &first.session).await,
                        Err(Error::ResourceExhausted { .. })
                    ));
                    assert_eq!(
                        f.storage.resume_file_once(&first_id, &token).await.unwrap(),
                        first
                    );
                    assert_eq!(f.data.file_generations.lock().unwrap()["bounded"], 2);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("receipts_replay_restart", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut file = f.storage.produce_file(metadata("restart")).await.unwrap();
                    file.write_all(b"saved").await.unwrap();
                    let token = file.session().unwrap().clone();
                    let id = identity(&f.storage, "resume");
                    let first = f.storage.resume_file_once(&id, &token).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let restored = super::recovery::restart(&f).await;
                    assert_eq!(
                        restored
                            .storage
                            .resume_file_once(&id, &token)
                            .await
                            .unwrap(),
                        first
                    );
                    assert_eq!(restored.data.file_generations.lock().unwrap()["restart"], 2);
                    let unknown = MutationId {
                        key: Identifier::new("unknown").unwrap(),
                        ..id
                    };
                    assert!(
                        restored
                            .storage
                            .resume_file_once(&unknown, &first.session)
                            .await
                            .is_err()
                    );
                    let fresh = identity(&restored.storage, "next");
                    assert_eq!(
                        restored
                            .storage
                            .resume_file_once(&fresh, &first.session)
                            .await
                            .unwrap()
                            .session
                            .generation,
                        3
                    );
                    restored.finish().await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("receipt_memory_checked", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.mutation_memory_bytes = NonZeroUsize::new(128).unwrap();
                    let f = Fixture::new(settings).await;
                    let mut file = f.storage.produce_file(metadata("memory")).await.unwrap();
                    let token = file.session().unwrap().clone();
                    let id = identity(&f.storage, "bounded");
                    assert!(matches!(
                        f.storage.resume_file_once(&id, &token).await,
                        Err(Error::ResourceExhausted { .. })
                    ));
                    assert_eq!(f.storage.status().mutation_bytes, 0);
                    assert_eq!(f.data.file_generations.lock().unwrap()["memory"], 1);
                    file.write_all(b"still owned").await.unwrap();
                    file.abort().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("matching_resumes_share", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let f = Fixture::new(config()).await;
                        let file = f
                            .storage
                            .produce_file(metadata("concurrent"))
                            .await
                            .unwrap();
                        let token = file.session().unwrap().clone();
                        let id = identity(&f.storage, "shared");
                        f.data.control.block_fence.store(true, Ordering::SeqCst);
                        let first = f.storage.resume_file_once(&id, &token);
                        let second = async {
                            f.data.control.fence_entered.notified().await;
                            let wait = f.storage.resume_file_once(&id, &token);
                            tokio::pin!(wait);
                            // Poll The Duplicate While The Original Physical Fence Is Still Held.
                            std::future::poll_fn(|context| {
                                assert!(
                                    std::future::Future::poll(wait.as_mut(), context).is_pending()
                                );
                                std::task::Poll::Ready(())
                            })
                            .await;
                            f.data.control.block_fence.store(false, Ordering::SeqCst);
                            f.data.control.fence_release.notify_one();
                            wait.await.unwrap()
                        };
                        let (first, second) = tokio::join!(first, second);
                        assert_eq!(first.unwrap(), second);
                        assert_eq!(second.session.generation, 2);
                        f.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("budgets_never_multiply", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.max_retries = 3;
                    let f = Fixture::new(settings).await;
                    Operations::register(
                        &f.runtime.operations,
                        "storage",
                        crate::operations::Lanes::Open,
                        Arc::new(f.storage.clone()),
                    )
                    .unwrap();
                    let file = f
                        .storage
                        .produce_file(metadata("retry-budget"))
                        .await
                        .unwrap();
                    let request = FileMutation {
                        mutation: identity(&f.storage, "resume"),
                        session: file.session().unwrap().clone(),
                    };
                    f.data.control.fence_failures.store(5, Ordering::SeqCst);
                    assert!(
                        f.runtime
                            .operations
                            .call::<ResumeFile>(request.clone())
                            .await
                            .is_err()
                    );
                    assert_eq!(f.data.control.fence_failures.load(Ordering::SeqCst), 1);
                    f.data.control.fence_failures.store(0, Ordering::SeqCst);
                    assert_eq!(
                        f.runtime
                            .operations
                            .call::<ResumeFile>(request)
                            .await
                            .unwrap()
                            .session
                            .generation,
                        2
                    );
                    f.finish().await;
                });
        });
    }
}
