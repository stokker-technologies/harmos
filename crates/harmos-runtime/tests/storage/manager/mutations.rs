/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use harmos_operations::storage::CreateFileWriterRequest;

// Standard Library Imports
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::{Fixture, config, metadata};
use harmos_runtime::Identifier;
use harmos_runtime::storage::{MutationId, StorageManager};

/* === Private Functions === */

fn identity(storage: &StorageManager, key: &str) -> MutationId {
    MutationId {
        window: storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}

/* === Tests === */

#[tokio::test]
async fn resume_receipt_replays() {
    let f = Fixture::new(config()).await;
    let mut file = f.storage.produce_file(metadata("once")).await.unwrap();
    file.write_all((b"prefix").to_vec()).await.unwrap();
    let token = file.session().unwrap().clone();
    let id = identity(&f.storage, "resume");
    let first = f.storage.resume_file_once(&id, &token).await.unwrap();
    assert_eq!(first.length, 6);
    assert_eq!(first.session.generation, 2);
    assert_eq!(f.storage.resume_file_once(&id, &token).await.unwrap(), first);
    assert!(file.write_all((b"stale").to_vec()).await.is_err());
    let mut later = f.storage.resume_file(&first.session).await.unwrap();
    later.write_all((b"tail").to_vec()).await.unwrap();
    assert_eq!(f.storage.resume_file_once(&id, &token).await.unwrap(), first);
    assert!(f.storage.resume_file_once(&id, later.session().unwrap()).await.is_err());
    assert_eq!(later.session().unwrap().generation, 3);
    later.abort().await.unwrap();
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn operation_retries_three() {
    let f = Fixture::new(config()).await;
    crate::support::app::register(&f.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let file = f.storage.produce_file(metadata("retry")).await.unwrap();
    let request = CreateFileWriterRequest {
        mutation: Some(identity(&f.storage, "resume")),
        session: Some(file.session().unwrap().clone()),
    };
    f.data.control.fence_failures.store(3, Ordering::SeqCst);
    let first = f.runtime.operations.call_native::<harmos_operations::storage::CreateFileWriter>(request.clone()).await.unwrap();
    assert_eq!(first.production.as_ref().unwrap().session.generation, 2);
    assert_eq!(f.data.control.fence_failures.load(Ordering::SeqCst), 0);
    assert_eq!(f.runtime.operations.call_native::<harmos_operations::storage::CreateFileWriter>(request).await.unwrap(), first);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_resume_replays() {
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
}

#[tokio::test]
async fn duplicate_resumes_share() {
    let f = Fixture::new(config()).await;
    let file = f.storage.produce_file(metadata("concurrent")).await.unwrap();
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
}

#[tokio::test(start_paused = true)]
async fn budgets_never_multiply() {
    let f = Fixture::new(config()).await;
    crate::support::app::register(&f.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let file = f.storage.produce_file(metadata("retry-budget")).await.unwrap();
    let request = CreateFileWriterRequest {
        mutation: Some(identity(&f.storage, "resume")),
        session: Some(file.session().unwrap().clone()),
    };
    f.data.control.fence_failures.store(5, Ordering::SeqCst);
    assert!(f.runtime.operations.call_native::<harmos_operations::storage::CreateFileWriter>(request.clone()).await.is_err());
    assert_eq!(f.data.control.fence_failures.load(Ordering::SeqCst), 1);
    f.data.control.fence_failures.store(0, Ordering::SeqCst);
    assert_eq!(
        f.runtime
            .operations
            .call_native::<harmos_operations::storage::CreateFileWriter>(request)
            .await
            .unwrap()
            .production
            .unwrap()
            .session
            .generation,
        2
    );
    f.finish().await;
}
