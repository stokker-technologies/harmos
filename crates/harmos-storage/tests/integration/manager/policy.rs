/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::fixture::{Fixture, config, start, write};
use harmos_storage::config::{Backoff, Jitter, RetryPolicy};
use harmos_storage::{BackendFailure, Error, FileMetadata, Identifier, MutationId, StagedFile, StorageManager};
use std::{num::NonZeroUsize, sync::atomic::Ordering, time::Duration};

/* === Tests === */

#[tokio::test]
async fn configured_policy_owns_io_retries_and_retirement() {
    let clock = super::clock::Controlled::new();
    let mut settings = config();
    let io = Duration::from_secs(2);
    let recovery_open = Duration::from_secs(5);
    let backoff = Duration::from_millis(17);
    let session = Duration::from_secs(7);
    settings.policy.io_timeout = io;
    settings.policy.recovery_timeout = recovery_open;
    settings.policy.retries = RetryPolicy::OnTransient {
        total_attempts: 2,
        backoff: Backoff::Fixed { delay: backoff, jitter: Jitter::None },
    };
    settings.policy.file_session_timeout = session;
    settings.policy.mutation_retention = Duration::from_secs(20);
    settings.request.max_file_productions = NonZeroUsize::MIN;
    settings.request.max_mutation_receipts = NonZeroUsize::new(4).unwrap();
    let fixture =
        Fixture::build_with(settings, |backends, settings| StorageManager::with_execution(backends, settings, clock.source()).unwrap());

    // Recovery Opening Has Its Own Configured Budget.
    fixture.recovery.control.block_open.store(true, Ordering::Release);
    let mut opening = Box::pin(start(&fixture.storage));
    assert!(super::events::until(opening.as_mut(), fixture.recovery.control.open_entered.notified()).await.is_ok());
    clock.scheduled(recovery_open).await;
    clock.advance(io);
    assert!(
        std::future::poll_fn(|context| std::task::Poll::Ready(std::future::Future::poll(opening.as_mut(), context).is_pending())).await
    );
    fixture.recovery.control.block_open.store(false, Ordering::Release);
    fixture.recovery.control.open_release.notify_one();
    opening.await;

    // Ordinary I/O Times Out On The Same Controlled Source, Then Retries Only Its Credit.
    fixture.data.control.block_write.store(true, Ordering::Release);
    let position = write(&fixture.storage, "admitted").await;
    let mut flush = Box::pin(fixture.storage.flush_until(&position));
    assert!(super::events::until(flush.as_mut(), fixture.data.control.write_entered.notified()).await.is_ok());
    clock.scheduled(io).await;
    clock.advance(io);
    fixture.data.control.write_settled.notified().await;
    assert_eq!(flush.await.unwrap_err().backend_failure(), Some(BackendFailure::Transient));
    clock.scheduled(backoff).await;
    clock.advance(backoff);
    fixture.data.control.write_entered.notified().await;
    clock.scheduled(io).await;
    clock.advance(io);
    fixture.data.control.write_settled.notified().await;
    clock.advance(io);
    assert_eq!(fixture.data.control.writes.lock().unwrap().as_slice(), &[position.sequence, position.sequence]);
    fixture.data.control.block_write.store(false, Ordering::Release);
    fixture.storage.retry_transient_persistence();
    fixture.storage.flush_until(&position).await.unwrap();
    assert_eq!(fixture.storage.get_saved_position(), position);
    assert_eq!(fixture.data.records.lock().unwrap().len(), 1);

    // Production Admission, Idle Lifetime And Receipt Lifetime Are Independent.
    let metadata = |name: &str| FileMetadata {
        id: Identifier::new(name).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    };
    let window = fixture.storage.mutation_window().unwrap();
    let mutation = MutationId {
        window: window.id.clone(),
        key: Identifier::new("production").unwrap(),
    };
    let producer = fixture.storage.create_file_once(&mutation, &metadata("session")).await.unwrap();
    let second = MutationId {
        window: window.id,
        key: Identifier::new("capacity").unwrap(),
    };
    assert!(matches!(fixture.storage.create_file_once(&second, &metadata("other")).await, Err(Error::ResourceExhausted { .. })));
    fixture.data.control.block_abort.store(true, Ordering::Release);
    clock.advance(session);
    fixture.data.control.abort_entered.notified().await;
    assert!(fixture.storage.resume_file(&producer.session).await.is_err());
    fixture.data.control.block_abort.store(false, Ordering::Release);
    fixture.data.control.abort_release.notify_one();
    fixture.data.control.abort_settled.notified().await;
    assert_eq!(fixture.storage.create_file_once(&mutation, &metadata("session")).await.unwrap(), producer);
    let remaining = window.expires_at.duration_since(fixture.storage.clock().read().date).unwrap();
    clock.advance(remaining);
    assert!(fixture.storage.create_file_once(&mutation, &metadata("session")).await.is_err());
    let refreshed = MutationId {
        window: fixture.storage.mutation_window().unwrap().id,
        key: Identifier::new("new-production").unwrap(),
    };
    let next = fixture.storage.create_file_once(&refreshed, &metadata("next")).await.unwrap();
    let file = StagedFile {
        history: next.session.history.clone(),
        id: next.session.id.clone(),
    };
    assert_eq!(fixture.storage.get_file_production_status(&file).await.unwrap().accepted_length, 0);
    fixture.finish().await;
}
