/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// File Mutations Wait for the Shared Write Budget in Arrival Order

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::Poll;

// Third Party Imports
use harmos_query::index::IndexDefinition;
use tokio::sync::Notify;

// Local Imports
use super::events;
use super::fixture::{Fixture, config, schema};
use harmos_storage::{Error, FileMetadata, FileProducer, FileSession, Identifier, MutationId, ReadLimits, StorageSettings};

/* === Constants === */

const BUDGET: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn settings() -> StorageSettings {
    let mut settings = config();
    settings.request.max_pending_write_bytes = NonZeroUsize::new(BUDGET).unwrap();
    settings
}

fn identity(f: &Fixture, key: &str) -> MutationId {
    MutationId {
        window: f.storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}

async fn producer(f: &Fixture, name: &str) -> FileSession {
    let metadata = FileMetadata {
        id: Identifier::new(name).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    };
    f.storage.create_file_once(&identity(f, &format!("create-{name}")), &metadata).await.unwrap().session
}

async fn append(f: &Fixture, key: &str, session: &FileSession, bytes: Vec<u8>) -> harmos_storage::Result<FileProducer> {
    f.storage.append_file_once(&identity(f, key), session, 0, bytes).await
}

async fn poll_once<F: Future + ?Sized>(mut future: Pin<&mut F>) -> Poll<F::Output> {
    std::future::poll_fn(|context| Poll::Ready(future.as_mut().poll(context))).await
}

fn pending(f: &Fixture) -> (usize, usize) {
    let status = f.storage.status();
    (status.pending_bytes, status.pending_batches)
}

/* === Tests === */

#[tokio::test]
async fn a_waiter_past_its_deadline_leaves_and_the_next_proceeds_once_capacity_frees() {
    let f = Fixture::new(settings()).await;
    let holder = producer(&f, "holder").await;
    let late = producer(&f, "late").await;
    let patient = producer(&f, "patient").await;
    assert_eq!(pending(&f), (0, 0));

    // A Slow Backend Holds the Whole Budget With One Admitted Chunk.
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    *f.data.control.block_chunk_file.lock().unwrap() = Some("holder".into());
    let mut hold = Box::pin(append(&f, "hold", &holder, vec![1; BUDGET]));
    assert!(events::until(hold.as_mut(), f.data.control.chunk_entered.notified()).await.is_ok());
    assert_eq!(pending(&f), (BUDGET, 1));

    // One Waiter's Caller Deadline Ends Its Wait; It Took No Share and Leaves the Queue.
    let deadline = Notify::new();
    deadline.notify_one();
    {
        let mut abandoned = Box::pin(append(&f, "late", &late, vec![2; 8]));
        assert!(events::until(abandoned.as_mut(), deadline.notified()).await.is_ok());
    }
    assert_eq!(pending(&f), (BUDGET, 1));

    // The Next Waiter Is Not Refused; It Proceeds Once the Held Chunk Drains.
    let mut waiting = Box::pin(append(&f, "patient", &patient, vec![3; 8]));
    assert!(poll_once(waiting.as_mut()).await.is_pending());
    assert_eq!(pending(&f), (BUDGET, 1));
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_one();
    let (held, waited) = tokio::join!(hold, waiting);
    assert_eq!(held.unwrap().length, BUDGET as u64);
    assert_eq!(waited.unwrap().length, 8);

    // The Abandoned Identity Was Never Admitted, so Its Retry Runs Afresh.
    assert_eq!(append(&f, "late", &late, vec![2; 8]).await.unwrap().length, 8);
    assert_eq!(pending(&f), (0, 0));
    f.finish().await;
}

#[tokio::test]
async fn a_large_command_is_served_before_later_small_ones() {
    let f = Fixture::new(settings()).await;
    let holder = producer(&f, "holder").await;
    let large = producer(&f, "large").await;
    let small = producer(&f, "small").await;

    // Half the Budget Is Held; the Large Command Needs All of It and Queues First.
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    *f.data.control.block_chunk_file.lock().unwrap() = Some("holder".into());
    let mut hold = Box::pin(append(&f, "hold", &holder, vec![1; BUDGET / 2]));
    assert!(events::until(hold.as_mut(), f.data.control.chunk_entered.notified()).await.is_ok());
    let mut first = Box::pin(append(&f, "large", &large, vec![2; BUDGET]));
    assert!(poll_once(first.as_mut()).await.is_pending());

    // A Later Small Command Would Fit, Yet Waits Behind the Large One.
    let mut second = Box::pin(append(&f, "small", &small, vec![3; 8]));
    assert!(poll_once(second.as_mut()).await.is_pending());
    assert_eq!(pending(&f), (BUDGET / 2, 1));

    // Once the Holder Drains, the Large Command Takes the Whole Budget First.
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_one();
    hold.await.unwrap();
    assert_eq!(pending(&f), (0, 0));
    assert!(poll_once(second.as_mut()).await.is_pending());
    assert_eq!(pending(&f), (0, 0));
    assert!(poll_once(first.as_mut()).await.is_pending());
    assert_eq!(pending(&f), (BUDGET, 1));
    assert!(poll_once(second.as_mut()).await.is_pending());
    assert_eq!(pending(&f), (BUDGET, 1));
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.unwrap().length, BUDGET as u64);
    assert_eq!(second.unwrap().length, 8);
    assert_eq!(pending(&f), (0, 0));
    f.finish().await;
}

#[tokio::test]
async fn a_command_larger_than_the_whole_budget_is_refused_at_once() {
    let f = Fixture::new(settings()).await;
    let holder = producer(&f, "holder").await;
    let oversized = producer(&f, "oversized").await;
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    let mut hold = Box::pin(append(&f, "hold", &holder, vec![1; BUDGET]));
    assert!(events::until(hold.as_mut(), f.data.control.chunk_entered.notified()).await.is_ok());

    // No Amount of Waiting Fits It, so It Fails on Its First Attempt Without Taking a Share.
    let mut refused = Box::pin(append(&f, "oversized", &oversized, vec![2; BUDGET + 1]));
    assert!(matches!(poll_once(refused.as_mut()).await, Poll::Ready(Err(Error::InvalidInput { field: "file mutation", .. }))));
    drop(refused);
    assert_eq!(pending(&f), (BUDGET, 1));
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_one();
    hold.await.unwrap();
    f.finish().await;
}

#[tokio::test]
async fn a_definition_admitted_beside_an_exclusive_permit_releases_its_share() {
    let f = Fixture::new(config()).await;
    let definition = IndexDefinition {
        name: Identifier::new("by-item").unwrap(),
        version: 1,
        source: harmos_query::columnar::ColumnSource {
            kind: harmos_query::columnar::ColumnKind::Records,
            schema: schema(),
        },
        columns: vec!["item".into()],
    };
    let work = ReadLimits {
        rows: NonZeroUsize::new(32).unwrap(),
        bytes: NonZeroUsize::new(64 * 1024).unwrap(),
    };

    // A Definition Joins While a Recovery Permit Holds the Exclusive Reservation.
    let permit = f.storage.reserve_snapshot(16).await.unwrap();
    f.data.control.block_definition.store(true, Ordering::SeqCst);
    let index = identity(&f, "index");
    let mut set = Box::pin(f.storage.set_index_definition_once(&index, definition, None, false, work));
    assert!(events::until(set.as_mut(), f.data.control.definition_entered.notified()).await.is_ok());

    // The Permit Ends First; the Definition Still Owns Its Share Until It Completes.
    drop(permit);
    assert_eq!(pending(&f).1, 1);
    f.data.control.block_definition.store(false, Ordering::SeqCst);
    f.data.control.definition_release.notify_one();
    set.await.unwrap();
    assert_eq!(pending(&f), (0, 0));
    f.finish().await;
}
