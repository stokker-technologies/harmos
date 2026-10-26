/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Verify Admitted Mutation Completion Across Retry Expiry

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::cell::Cell;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;
use tokio::time::Instant;

// Local Imports
use super::backend::{Control, Data, Recovery, history};
use super::fixture::{Fixture, config, pending, start};
use harmos_storage::execution::{Execution, Sleep};
use harmos_storage::{FileMetadata, FileSession, Identifier, MutationId, MutationValue, StagedFile, StorageBackends, StorageManager};

/* === Constants === */

/* === Macros === */

thread_local! {
    static ELAPSED: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Clone, Copy, Debug)]
enum Work {
    Append,
    Resume,
    Finish,
    Abandon,
    Delete,
}

/* === Traits === */

/* === Implementations === */

impl Work {
    fn gate(self, control: &Control) -> (&AtomicBool, &Notify, &Notify) {
        match self {
            Self::Append => (&control.block_chunk, &control.chunk_entered, &control.chunk_release),
            Self::Resume => (&control.block_fence, &control.fence_entered, &control.fence_release),
            Self::Finish => (&control.block_finish, &control.finish_entered, &control.finish_release),
            Self::Abandon => (&control.block_abort, &control.abort_entered, &control.abort_release),
            Self::Delete => (&control.block_delete, &control.delete_entered, &control.delete_release),
        }
    }

    async fn call(
        self, storage: &StorageManager, mutation: &MutationId, session: &FileSession, file: &StagedFile,
    ) -> harmos_storage::Result<MutationValue> {
        match self {
            Self::Append => storage.append_file_once(mutation, session, 3, (b"tail").to_vec()).await.map(MutationValue::Producer),
            Self::Resume => storage.resume_file_once(mutation, session).await.map(MutationValue::Producer),
            Self::Finish => {
                let reference = storage.finish_file_once(mutation, session).await?;
                let reference = Box::new(reference);
                Ok(MutationValue::Reference(reference))
            }
            Self::Abandon => storage.abandon_file_once(mutation, session).await.map(MutationValue::Progress),
            Self::Delete => storage.delete_file_once(mutation, file).await.map(MutationValue::Deleted),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn now() -> Instant {
    Instant::now() + ELAPSED.with(Cell::get)
}

fn sleep(duration: Duration) -> Sleep {
    Box::pin(tokio::time::sleep(duration))
}

async fn fixture() -> Fixture {
    set_elapsed(0);
    let store_history = history();
    let data = Data::new(store_history.clone());
    let recovery = Recovery::new(store_history);
    let data = Arc::new(data);
    let recovery = Arc::new(recovery);

    let mut settings = config();
    settings.budget.read_memory_bytes = NonZeroUsize::new(128 * 1024).unwrap();
    let storage = StorageManager::with_execution(
        StorageBackends { primary: data.clone(), state: recovery.clone() },
        settings,
        Execution::new(now, sleep),
    )
    .unwrap();
    start(&storage).await;
    Fixture { storage, data, recovery }
}

fn set_elapsed(seconds: u64) {
    let elapsed = Duration::from_secs(seconds);
    ELAPSED.with(|clock| clock.set(elapsed));
}

fn metadata(id: &str) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(id).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    }
}

fn identity(storage: &StorageManager, key: &str) -> MutationId {
    let window = storage.mutation_window().unwrap().id;
    let key = Identifier::new(key).unwrap();
    MutationId { window, key }
}

/* === Tests === */

#[tokio::test]
async fn admitted_create_completes() {
    let f = fixture().await;
    let mutation = identity(&f.storage, "create");
    set_elapsed(299);
    let metadata = metadata("admitted");
    f.data.control.block_create.store(true, Ordering::SeqCst);

    let result;
    {
        let original = f.storage.create_file_once(&mutation, &metadata);
        tokio::pin!(original);
        assert!(pending(original.as_mut()).await);
        f.data.control.create_entered.notified().await;
        let staged = f.data.staged.lock().unwrap().len();
        assert_eq!(staged, 1, "the backend staged the file before the clock crosses expiry");
        let retained = f.storage.status().mutation_bytes;
        assert!(retained > 0);
        set_elapsed(301);
        assert_ne!(f.storage.mutation_window().unwrap().id, mutation.window);
        assert!(f.storage.create_file_once(&mutation, &metadata).await.is_err());
        f.data.control.block_create.store(false, Ordering::SeqCst);
        f.data.control.create_release.notify_one();
        result = original.await;
    }
    let staged = f.data.staged.lock().unwrap().len();
    f.storage.mutation_window().unwrap();
    let reclaimed = f.storage.status().mutation_bytes;
    let late = f.storage.create_file_once(&mutation, &metadata).await;
    f.finish().await;
    assert_eq!(staged, 1, "one backend creation completed");
    assert!(result.is_ok(), "an admitted create keeps its completion: {result:?}");
    assert!(late.is_err(), "a new expired retry remains refused");
    assert_eq!(reclaimed, 0, "expired completion is reclaimed after its caller releases it");
}

#[tokio::test]
async fn attached_mutations_complete() {
    for work in [Work::Append, Work::Resume, Work::Finish, Work::Abandon, Work::Delete] {
        let f = fixture().await;
        let mutation = identity(&f.storage, "attached");
        set_elapsed(299);
        let mut writer = f.storage.produce_file(metadata("shared")).await.unwrap();
        writer.write_all((b"abc").to_vec()).await.unwrap();
        let session = writer.session().unwrap().clone();
        let file = StagedFile { history: session.history.clone(), id: session.id.clone() };
        if matches!(work, Work::Delete) {
            writer.finish().await.unwrap();
            f.storage.flush().await.unwrap();
        }

        let (blocked, entered, release) = work.gate(&f.data.control);
        blocked.store(true, Ordering::SeqCst);
        let mut original = Box::pin(work.call(&f.storage, &mutation, &session, &file));
        assert!(pending(original.as_mut()).await, "{work:?}");
        entered.notified().await;
        let mut attached = Box::pin(work.call(&f.storage, &mutation, &session, &file));
        assert!(pending(attached.as_mut()).await, "{work:?}");
        drop(original);

        set_elapsed(301);
        f.storage.mutation_window().unwrap();
        assert!(work.call(&f.storage, &mutation, &session, &file).await.is_err(), "{work:?}: late caller");
        blocked.store(false, Ordering::SeqCst);
        release.notify_one();
        let result = attached.await;

        f.storage.mutation_window().unwrap();
        let reclaimed = f.storage.status().mutation_bytes;
        let late = work.call(&f.storage, &mutation, &session, &file).await;
        drop(writer);
        f.finish().await;
        let value = result.unwrap_or_else(|error| panic!("{work:?}: attached completion lost: {error}"));
        match value {
            MutationValue::Producer(producer) if matches!(work, Work::Append) => assert_eq!(producer.length, 7),
            MutationValue::Producer(producer) => assert_eq!(producer.session.generation, 2),
            MutationValue::Reference(reference) => assert_eq!(reference.length, 3),
            MutationValue::Progress(progress) => assert_eq!(progress.length, 3),
            MutationValue::Deleted(deleted) => assert_eq!(deleted, file),
            other => panic!("unexpected {work:?} completion: {other:?}"),
        }
        assert!(late.is_err(), "{work:?}: expired completed identity remains refused");
        assert_eq!(reclaimed, 0, "{work:?}: cancelled and completed owners release their charge");
    }
}

#[tokio::test]
async fn completed_owner_retains() {
    let f = fixture().await;
    let first = identity(&f.storage, "first");
    set_elapsed(299);
    let first_metadata = metadata("first");
    f.data.control.block_create.store(true, Ordering::SeqCst);
    let mut waiting = Box::pin(f.storage.create_file_once(&first, &first_metadata));
    assert!(pending(waiting.as_mut()).await);
    f.data.control.create_entered.notified().await;
    let first_bytes = f.storage.status().mutation_bytes;

    f.data.control.block_create.store(false, Ordering::SeqCst);
    f.data.control.create_release.notify_one();
    for index in 0..24 {
        let name = format!("later-{index}");
        let later = identity(&f.storage, &name);
        let producer = f.storage.create_file_once(&later, &metadata(&name)).await.unwrap();
        let ending = identity(&f.storage, &format!("{name}-end"));
        f.storage.abandon_file_once(&ending, &producer.session).await.unwrap();
        assert!(f.storage.status().mutation_bytes <= 128 * 1024 / 8, "all completion ownership remains charged to its ledger budget");
    }
    assert!(f.storage.status().mutation_bytes > first_bytes);

    set_elapsed(301);
    f.storage.mutation_window().unwrap();
    assert_eq!(f.storage.status().mutation_bytes, first_bytes, "pinned front does not retain later expired entries");
    assert!(f.storage.create_file_once(&first, &first_metadata).await.is_err());
    let result = waiting.await;
    f.storage.mutation_window().unwrap();
    let reclaimed = f.storage.status().mutation_bytes;
    f.finish().await;
    assert!(result.is_ok(), "completed outcome survives purge until its caller polls it");
    assert_eq!(reclaimed, 0);
}

#[tokio::test]
async fn cancelled_owner_reclaims() {
    let f = fixture().await;
    let mutation = identity(&f.storage, "cancelled");
    set_elapsed(299);
    let original_metadata = metadata("cancelled");
    f.data.control.block_create.store(true, Ordering::SeqCst);
    let mut original = Box::pin(f.storage.create_file_once(&mutation, &original_metadata));
    assert!(pending(original.as_mut()).await);
    f.data.control.create_entered.notified().await;
    drop(original);

    set_elapsed(301);
    f.storage.mutation_window().unwrap();
    assert!(f.storage.status().mutation_bytes > 0, "pending backend work retains its reserved outcome");
    f.data.control.block_create.store(false, Ordering::SeqCst);
    f.data.control.create_release.notify_one();
    let next = identity(&f.storage, "next");
    f.storage.create_file_once(&next, &metadata("next")).await.unwrap();
    let retained = f.storage.status().mutation_bytes;

    set_elapsed(602);
    f.storage.mutation_window().unwrap();
    let reclaimed = f.storage.status().mutation_bytes;
    let late = f.storage.create_file_once(&mutation, &original_metadata).await;
    f.finish().await;
    assert!(retained > 0, "only the newer completed receipt remains before its own expiry");
    assert_eq!(reclaimed, 0, "cancelled completion does not leak a pending entry");
    assert!(late.is_err());
}

#[tokio::test]
async fn admitted_failure_visible() {
    let f = fixture().await;
    let mutation = identity(&f.storage, "failure");
    set_elapsed(299);
    let metadata = metadata("failure");
    f.data.control.block_create.store(true, Ordering::SeqCst);
    f.data.control.fail_create.store(true, Ordering::SeqCst);
    let mut original = Box::pin(f.storage.create_file_once(&mutation, &metadata));
    assert!(pending(original.as_mut()).await);
    f.data.control.create_entered.notified().await;

    set_elapsed(301);
    f.storage.mutation_window().unwrap();
    f.data.control.block_create.store(false, Ordering::SeqCst);
    f.data.control.create_release.notify_one();
    let result = original.await;
    f.data.control.fail_create.store(false, Ordering::SeqCst);
    f.finish().await;
    assert!(matches!(result, Err(harmos_storage::Error::Persistence { .. })), "backend failure remains visible: {result:?}");
}
