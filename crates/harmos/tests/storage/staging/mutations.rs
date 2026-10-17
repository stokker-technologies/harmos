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
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Fixture, config, metadata};
use harmos::Identifier;
use harmos::storage::operations::{FileMutation, ResumeFile};
use harmos::storage::{MutationId, StorageManager};

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

#[tokio::test]
async fn repeated_resume_returns() {
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
}

#[tokio::test(start_paused = true)]
async fn operation_retries_three() {
    let f = Fixture::new(config()).await;
    harmos::storage::operations::register(
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
        .call_as::<ResumeFile>(request.clone())
        .await
        .unwrap();
    assert_eq!(first.session.generation, 2);
    assert_eq!(f.data.control.fence_failures.load(Ordering::SeqCst), 0);
    assert_eq!(
        *f.runtime
            .operations
            .call_as::<ResumeFile>(request)
            .await
            .unwrap(),
        *first
    );
    f.finish().await;
}

#[tokio::test]
async fn cancelled_resume_wait() {
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
}

#[tokio::test]
async fn receipts_replay_restart() {
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
}

#[tokio::test]
async fn matching_resumes_share() {
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
                assert!(std::future::Future::poll(wait.as_mut(), context).is_pending());
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
}

#[tokio::test(start_paused = true)]
async fn budgets_never_multiply() {
    let f = Fixture::new(config()).await;
    harmos::storage::operations::register(
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
            .call_as::<ResumeFile>(request.clone())
            .await
            .is_err()
    );
    assert_eq!(f.data.control.fence_failures.load(Ordering::SeqCst), 1);
    f.data.control.fence_failures.store(0, Ordering::SeqCst);
    assert_eq!(
        f.runtime
            .operations
            .call_as::<ResumeFile>(request)
            .await
            .unwrap()
            .session
            .generation,
        2
    );
    f.finish().await;
}

#[tokio::test]
async fn thousands_mutations_accepted() {
    // A Small Budget Makes The Ledger Evict Completed Receipts Long Before The Window Ends.
    let mut settings = config();
    settings.budget.read_memory_bytes = std::num::NonZeroUsize::new(1024 * 1024).unwrap();
    settings.response.batch_bytes = std::num::NonZeroUsize::new(64 * 1024).unwrap();
    let budget = settings.budget.read_memory_bytes.get();
    let f = Fixture::new(settings).await;
    let window = f.storage.mutation_window().unwrap();
    let mut last = None;
    for index in 0..2048 {
        let name = format!("small-{index}");
        let create = identity(&f.storage, &format!("create-{index}"));
        let producer = f
            .storage
            .create_file_once(&create, &metadata(&name))
            .await
            .unwrap();
        let finish = identity(&f.storage, &format!("finish-{index}"));
        let reference = f
            .storage
            .finish_file_once(&finish, &producer.session)
            .await
            .unwrap();
        assert_eq!(reference.length, 0);
        last = Some((finish, producer.session, reference));
    }
    assert_eq!(f.storage.mutation_window().unwrap().id, window.id);
    assert!(f.storage.status().mutation_bytes < budget);

    // The Newest Receipt Still Answers Its Retry With The Original Outcome.
    let (finish, session, reference) = last.unwrap();
    assert_eq!(
        f.storage.finish_file_once(&finish, &session).await.unwrap(),
        reference
    );
    f.finish().await;
}
