/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Verify Admitted Mutation Completion Across Retry Expiry

/* === Imports === */

// Standard Library Imports
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::future::{self, Future};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;
use std::time::Duration;

// Third Party Imports
use tokio::sync::{Barrier, Notify};

// Local Imports
use super::backend::Backend;
use super::backend::RecoveryBackend;
use super::backend::history;
use super::fixture::{Fixture, config, record, schema, start};
use harmos_storage::memory::Control;
use harmos_storage::{
    BackendFailure, FileMetadata, FileSession, Identifier, MutationId, MutationValue, ObjectCatalog, ReadLimits, StagedFile,
    StorageBackend, StorageBackends, StorageManager,
};
use harmos_storage::{Error, Result};

/* === Macros === */

thread_local! {
    static CLOCK: RefCell<super::clock::Controlled> = RefCell::new(super::clock::Controlled::new());
    static ELAPSED: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

/* === Enums === */

#[derive(Clone, Copy, Debug)]
enum Work {
    Append,
    Resume,
    Finish,
    Abandon,
    Delete,
}

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
    ) -> Result<MutationValue> {
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

/* === Private Functions === */

async fn fixture() -> Fixture {
    set_elapsed(0);
    let store_history = history();
    let data = Backend::new(store_history.clone());
    let recovery = RecoveryBackend::new(store_history);
    let data = Arc::new(data);
    let recovery = Arc::new(recovery);

    let settings = config();

    let storage = StorageManager::with_execution(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        settings,
        CLOCK.with(|clock| clock.borrow().source()),
    )
    .unwrap();
    start(&storage).await;
    Fixture { storage, data, recovery }
}

fn set_elapsed(seconds: u64) {
    let elapsed = Duration::from_secs(seconds);
    ELAPSED.with(|previous| {
        CLOCK.with(|clock| {
            if elapsed.is_zero() {
                *clock.borrow_mut() = super::clock::Controlled::new();
            } else {
                clock.borrow().advance(elapsed - previous.get());
            }
        });
        previous.set(elapsed);
    });
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
async fn reserved_and_fresh_identities_survive_their_window_rotation() {
    let f = fixture().await;
    let reserved = identity(&f.storage, "reserved");
    let fresh = MutationId {
        window: reserved.window.clone(),
        key: Identifier::new("fresh").unwrap(),
    };
    let reserved_metadata = metadata("reserved");
    set_elapsed(149);
    f.data.control.block_receipt_lookup.store(true, Ordering::Release);
    let mut original = Box::pin(f.storage.create_file_once(&reserved, &reserved_metadata));
    assert!(super::events::until(original.as_mut(), f.data.control.receipt_lookup_entered.notified()).await.is_ok());
    set_elapsed(151);
    let current = identity(&f.storage, "current");
    assert_ne!(current.window, reserved.window);
    f.data.control.block_receipt_lookup.store(false, Ordering::Release);
    f.data.control.receipt_lookup_release.notify_one();
    let producer = original.await.unwrap();
    assert_eq!(f.storage.create_file_once(&reserved, &reserved_metadata).await.unwrap(), producer);
    f.storage.create_file_once(&fresh, &metadata("fresh")).await.unwrap();
    set_elapsed(300);
    let expired = MutationId {
        window: reserved.window,
        key: Identifier::new("expired").unwrap(),
    };
    assert!(matches!(
        f.storage.create_file_once(&expired, &metadata("expired")).await,
        Err(Error::Backend {
            kind: BackendFailure::MutationExpired,
            ..
        })
    ));
    assert!(matches!(
        f.storage.create_file_once(&fresh, &metadata("fresh")).await,
        Err(Error::Backend {
            kind: BackendFailure::MutationExpired,
            ..
        })
    ));
    f.storage.create_file_once(&current, &metadata("current")).await.unwrap();
    assert_eq!(f.data.staged.lock().unwrap().len(), 3);
    f.finish().await;
}

#[tokio::test]
async fn cache_eviction_preserves_the_window_and_durable_replay() {
    let clock = super::clock::Controlled::new();
    let history = history();
    let data = Arc::new(Backend::with_execution(ObjectCatalog::new(), history.clone(), clock.source()));
    let recovery = Arc::new(RecoveryBackend::new(history));
    let mut settings = config();
    settings.request.max_mutation_receipts = NonZeroUsize::new(4).unwrap();
    let storage =
        StorageManager::with_execution(StorageBackends { primary: data.clone(), state: recovery }, settings.clone(), clock.source())
            .unwrap();
    start(&storage).await;
    let window = storage.mutation_window().unwrap();
    let mut outcomes = Vec::new();
    let input = record(&schema(), &Identifier::new("event").unwrap(), &[1]);
    for index in 0..4 {
        let mutation = MutationId {
            window: window.id.clone(),
            key: Identifier::new(format!("seed-{index}")).unwrap(),
        };
        let position = storage.write_records_once(&mutation, vec![input.clone()]).await.unwrap();
        storage.flush_until(&position).await.unwrap();
        storage.wait_mutation_durable(&mutation, &position).await.unwrap();
        outcomes.push((mutation, position));
    }

    // Capture One Window Before Releasing Three Fresh Submitters.
    let barrier = Arc::new(Barrier::new(4));
    let mut submitters = Vec::new();
    for index in 0..3 {
        let storage = storage.clone();
        let barrier = barrier.clone();
        let input = input.clone();
        submitters.push(tokio::spawn(async move {
            let mutation = MutationId {
                window: storage.mutation_window().unwrap().id,
                key: Identifier::new(format!("fresh-{index}")).unwrap(),
            };
            barrier.wait().await;
            let position = storage.write_records_once(&mutation, vec![input]).await.unwrap();
            (mutation, position)
        }));
    }
    barrier.wait().await;
    let mut positions = BTreeSet::new();
    for submitter in submitters {
        let (mutation, position) = submitter.await.unwrap();
        assert_eq!(mutation.window, window.id);
        positions.insert(position.sequence);
        outcomes.push((mutation, position));
    }
    assert_eq!(positions, BTreeSet::from([5, 6, 7]));
    assert_eq!(storage.mutation_window().unwrap(), window);
    storage.flush().await.unwrap();
    for (mutation, position) in &outcomes {
        if mutation.key.as_str().starts_with("fresh-") {
            storage.wait_mutation_durable(mutation, position).await.unwrap();
        }
    }

    // An Evicted Identity Replays Its Position and Still Detects Changed Payloads.
    let accepted = storage.status().accepted;
    let (mutation, position) = &outcomes[0];
    assert_eq!(storage.write_records_once(mutation, vec![input.clone()]).await.unwrap(), *position);
    let mut changed = input.clone();
    changed.payload = vec![2];
    let failure = storage.write_records_once(mutation, vec![changed]).await.unwrap_err();
    assert_eq!(failure.backend_failure(), Some(BackendFailure::Conflict));
    assert_eq!(storage.status().accepted, accepted);
    assert_eq!(data.records.lock().unwrap().len(), 7);

    // Bounded Recovery Omits History That Remains Available Through Keyed Lookup.
    let limits = ReadLimits {
        rows: NonZeroUsize::new(4).unwrap(),
        bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
    };
    let recovered = data.list_mutation_receipts(limits).await.unwrap();
    let (omitted, original) = outcomes.iter().find(|(id, _)| !recovered.iter().any(|receipt| &receipt.mutation == id)).unwrap();
    let reopened = storage.new_session().unwrap();
    storage.close().await.unwrap();
    start(&reopened).await;
    assert_eq!(reopened.write_records_once(omitted, vec![input.clone()]).await.unwrap(), *original);
    assert_eq!(reopened.status().accepted, accepted);
    clock.advance(settings.policy.mutation_retention);
    assert!(reopened.write_records_once(omitted, vec![input.clone()]).await.is_err());
    let expired = MutationId {
        window: window.id,
        key: Identifier::new("never-admitted").unwrap(),
    };
    assert!(reopened.write_records_once(&expired, vec![input]).await.is_err());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn concurrent_duplicate_cache_misses_share_lookup_and_execution() {
    let f = Fixture::new(config()).await;
    let mutation = identity(&f.storage, "duplicate-miss");
    let metadata = metadata("duplicate-miss");
    f.data.control.block_receipt_lookup.store(true, Ordering::Release);
    let mut original = Box::pin(f.storage.create_file_once(&mutation, &metadata));
    assert!(super::events::until(original.as_mut(), f.data.control.receipt_lookup_entered.notified()).await.is_ok());
    let mut duplicate = Box::pin(f.storage.create_file_once(&mutation, &metadata));
    assert!(future::poll_fn(|context| Poll::Ready(Future::poll(duplicate.as_mut(), context).is_pending())).await);
    f.data.control.block_receipt_lookup.store(false, Ordering::Release);
    f.data.control.receipt_lookup_release.notify_one();
    let (original, duplicate) = tokio::join!(original, duplicate);
    assert_eq!(original.unwrap(), duplicate.unwrap());
    assert_eq!(f.data.control.receipt_lookups.load(Ordering::Acquire), 1);
    assert_eq!(f.data.staged.lock().unwrap().len(), 1);
    f.finish().await;
}

#[tokio::test]
async fn failed_lookup_never_admits_fresh_work() {
    let f = Fixture::new(config()).await;
    let mutation = identity(&f.storage, "lookup-failure");
    let metadata = metadata("lookup-failure");
    f.data.control.fail_receipt_lookup.store(true, Ordering::Release);
    assert!(f.storage.create_file_once(&mutation, &metadata).await.is_err());
    assert!(f.data.staged.lock().unwrap().is_empty());
    f.data.control.fail_receipt_lookup.store(false, Ordering::Release);
    f.storage.create_file_once(&mutation, &metadata).await.unwrap();
    assert_eq!(f.data.staged.lock().unwrap().len(), 1);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_lookup_releases_its_pending_entry() {
    let f = Fixture::new(config()).await;
    let mutation = identity(&f.storage, "cancelled-lookup");
    let metadata = metadata("cancelled-lookup");
    f.data.control.block_receipt_lookup.store(true, Ordering::Release);
    let mut original = Box::pin(f.storage.create_file_once(&mutation, &metadata));
    assert!(super::events::until(original.as_mut(), f.data.control.receipt_lookup_entered.notified()).await.is_ok());
    drop(original);
    f.data.control.block_receipt_lookup.store(false, Ordering::Release);
    f.storage.create_file_once(&mutation, &metadata).await.unwrap();
    assert_eq!(f.data.control.receipt_lookups.load(Ordering::Acquire), 2);
    assert_eq!(f.data.staged.lock().unwrap().len(), 1);
    f.finish().await;
}

#[tokio::test]
async fn closing_storage_refuses_a_pending_lookup() {
    let f = Fixture::new(config()).await;
    let mutation = identity(&f.storage, "closing-lookup");
    let metadata = metadata("closing-lookup");
    f.data.control.block_receipt_lookup.store(true, Ordering::Release);
    let mut original = Box::pin(f.storage.create_file_once(&mutation, &metadata));
    assert!(super::events::until(original.as_mut(), f.data.control.receipt_lookup_entered.notified()).await.is_ok());
    f.storage.close().await.unwrap();
    f.data.control.block_receipt_lookup.store(false, Ordering::Release);
    f.data.control.receipt_lookup_release.notify_one();
    assert!(original.await.is_err());
    assert!(f.data.staged.lock().unwrap().is_empty());
}

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
        assert!(super::events::until(original.as_mut(), f.data.control.create_entered.notified()).await.is_ok());
        let staged = f.data.staged.lock().unwrap().len();
        assert_eq!(staged, 1, "the backend staged the file before the clock crosses expiry");
        set_elapsed(301);
        assert_ne!(f.storage.mutation_window().unwrap().id, mutation.window);
        assert!(f.storage.create_file_once(&mutation, &metadata).await.is_err());
        f.data.control.block_create.store(false, Ordering::SeqCst);
        f.data.control.create_release.notify_one();
        result = original.await;
    }
    let staged = f.data.staged.lock().unwrap().len();
    f.storage.mutation_window().unwrap();
    let late = f.storage.create_file_once(&mutation, &metadata).await;
    f.finish().await;
    assert_eq!(staged, 1, "one backend creation completed");
    assert!(result.is_ok(), "an admitted create keeps its completion: {result:?}");
    assert!(late.is_err(), "a new expired retry remains refused");
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
        let file = StagedFile {
            history: session.history.clone(),
            id: session.id.clone(),
        };
        if matches!(work, Work::Delete) {
            writer.finish().await.unwrap();
            f.storage.flush().await.unwrap();
        }

        let (blocked, entered, release) = work.gate(&f.data.control);
        blocked.store(true, Ordering::SeqCst);
        let mut original = Box::pin(work.call(&f.storage, &mutation, &session, &file));
        assert!(super::events::until(original.as_mut(), entered.notified()).await.is_ok());
        let mut attached = Box::pin(work.call(&f.storage, &mutation, &session, &file));
        assert!(future::poll_fn(|context| Poll::Ready(Future::poll(attached.as_mut(), context).is_pending())).await, "{work:?}");
        drop(original);

        set_elapsed(301);
        f.storage.mutation_window().unwrap();
        assert!(work.call(&f.storage, &mutation, &session, &file).await.is_err(), "{work:?}: late caller");
        blocked.store(false, Ordering::SeqCst);
        release.notify_one();
        let result = attached.await;

        f.storage.mutation_window().unwrap();
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
    assert!(super::events::until(waiting.as_mut(), f.data.control.create_entered.notified()).await.is_ok());

    f.data.control.block_create.store(false, Ordering::SeqCst);
    f.data.control.create_release.notify_one();
    for index in 0..24 {
        let name = format!("later-{index}");
        let later = identity(&f.storage, &name);
        let producer = f.storage.create_file_once(&later, &metadata(&name)).await.unwrap();
        let ending = identity(&f.storage, &format!("{name}-end"));
        f.storage.abandon_file_once(&ending, &producer.session).await.unwrap();
    }

    set_elapsed(301);
    f.storage.mutation_window().unwrap();
    assert!(f.storage.create_file_once(&first, &first_metadata).await.is_err());
    let result = waiting.await;
    f.storage.mutation_window().unwrap();
    f.finish().await;
    assert!(result.is_ok(), "completed outcome survives purge until its caller polls it");
}

#[tokio::test]
async fn cancelled_owner_reclaims() {
    let f = fixture().await;
    let mutation = identity(&f.storage, "cancelled");
    set_elapsed(299);
    let original_metadata = metadata("cancelled");
    f.data.control.block_create.store(true, Ordering::SeqCst);
    let mut original = Box::pin(f.storage.create_file_once(&mutation, &original_metadata));
    assert!(super::events::until(original.as_mut(), f.data.control.create_entered.notified()).await.is_ok());
    drop(original);

    set_elapsed(301);
    f.storage.mutation_window().unwrap();
    f.data.control.block_create.store(false, Ordering::SeqCst);
    f.data.control.create_release.notify_one();
    let next = identity(&f.storage, "next");
    f.storage.create_file_once(&next, &metadata("next")).await.unwrap();

    set_elapsed(602);
    f.storage.mutation_window().unwrap();
    let late = f.storage.create_file_once(&mutation, &original_metadata).await;
    f.finish().await;
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
    assert!(super::events::until(original.as_mut(), f.data.control.create_entered.notified()).await.is_ok());

    set_elapsed(301);
    f.storage.mutation_window().unwrap();
    f.data.control.block_create.store(false, Ordering::SeqCst);
    f.data.control.create_release.notify_one();
    let result = original.await;
    f.data.control.fail_create.store(false, Ordering::SeqCst);
    f.finish().await;
    assert!(matches!(result, Err(Error::Persistence { .. })), "backend failure remains visible: {result:?}");
}
