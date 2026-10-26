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
use std::num::NonZeroUsize;
use std::sync::{Arc, atomic::Ordering};

// Third Party Imports

// Local Imports
use super::{Fixture, config, metadata};
use harmos_runtime::Identifier;
use harmos_runtime::storage::operations::{AbandonFile, AppendFile, CreateFile, DeleteFile, FinishFile};
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
        crate::storage::operations::Lanes::Open,
        Arc::new(f.storage.clone()),
    )
    .unwrap();
    f
}
fn key(f: &Fixture, key: &str) -> MutationId {
    MutationId { window: f.storage.mutation_window().unwrap().id, key: Identifier::new(key).unwrap() }
}

/* === Tests === */

#[tokio::test]
async fn file_receipts_replay() {
    let f = fixture().await;
    let create = CreateFile { mutation: key(&f, "create"), metadata: metadata("shared") };
    let producer = f.runtime.operations.call(create.clone()).await.unwrap();
    assert_eq!(*f.runtime.operations.call(create.clone()).await.unwrap(), *producer);
    let first = AppendFile { mutation: key(&f, "first"), session: producer.session.clone(), offset: 0, bytes: b"one".to_vec() };
    let appended = f.runtime.operations.call(first.clone()).await.unwrap();
    assert_eq!(appended.length, 3);
    let second = AppendFile { mutation: key(&f, "second"), offset: 3, bytes: b"two".to_vec(), ..first.clone() };
    assert_eq!(f.runtime.operations.call(second).await.unwrap().length, 6);
    assert_eq!(*f.runtime.operations.call(first.clone()).await.unwrap(), *appended);
    let conflict = AppendFile { bytes: b"bad".to_vec(), ..first.clone() };
    assert!(f.runtime.operations.call(conflict).await.is_err());
    let wrong_offset = AppendFile { mutation: key(&f, "wrong-offset"), ..first };
    assert!(f.runtime.operations.call(wrong_offset).await.is_err());
    let finish = FinishFile { mutation: key(&f, "finish"), session: producer.session.clone() };
    let reference = f.runtime.operations.call(finish.clone()).await.unwrap();
    assert_eq!(reference.length, 6);
    assert_eq!(*f.runtime.operations.call(finish.clone()).await.unwrap(), *reference);
    assert_eq!(*f.runtime.operations.call(create).await.unwrap(), *producer);
    let identity = StagedFile { history: producer.session.history.clone(), id: producer.session.id.clone() };
    let mut reader = f.storage.follow_production(&identity, 0).await.unwrap();
    assert_eq!(reader.next_batch(NonZeroUsize::new(16).unwrap()).await.unwrap().unwrap().bytes(), b"onetwo");
    assert!(reader.next_batch(NonZeroUsize::MIN).await.unwrap().is_none());
    drop(reader);
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn lost_responses_retry() {
    let f = fixture().await;
    f.data.control.create_failures.store(3, Ordering::SeqCst);
    let producer = f.runtime.operations.call(CreateFile { mutation: key(&f, "create"), metadata: metadata("lost") }).await.unwrap();
    assert_eq!(producer.session.generation, 1);
    assert_eq!(f.data.control.create_failures.load(Ordering::SeqCst), 0);
    f.data.control.chunk_failures.store(3, Ordering::SeqCst);
    let append = AppendFile { mutation: key(&f, "append"), session: producer.session.clone(), offset: 0, bytes: b"partial".to_vec() };
    assert_eq!(f.runtime.operations.call(append.clone()).await.unwrap().length, 7);
    assert_eq!(*f.data.control.chunk_offsets.lock().unwrap(), vec![0, 0, 0, 0]);
    f.data.control.abandon_failures.store(3, Ordering::SeqCst);
    let abandon = AbandonFile { mutation: key(&f, "abandon"), session: producer.session.clone() };
    let abandoned = f.runtime.operations.call(abandon.clone()).await.unwrap();
    assert_eq!(abandoned.phase, FilePhase::Abandoned);
    assert_eq!(abandoned.length, 7);
    assert_eq!(*f.runtime.operations.call(abandon).await.unwrap(), *abandoned);
    assert_eq!(f.runtime.operations.call(append).await.unwrap().length, 7);
    assert_eq!(f.data.staged.lock().unwrap()["lost"].1, b"partial");
    f.data.control.delete_failures.store(3, Ordering::SeqCst);
    let delete = DeleteFile {
        mutation: key(&f, "delete"),
        file: StagedFile { history: producer.session.history.clone(), id: producer.session.id.clone() },
    };
    let deleted = f.runtime.operations.call(delete.clone()).await.unwrap();
    assert_eq!(*deleted, delete.file);
    assert_eq!(f.data.control.delete_failures.load(Ordering::SeqCst), 0);
    f.finish().await;
}

#[tokio::test]
async fn cancelled_append_replays() {
    let f = fixture().await;
    let producer = f.storage.create_file_once(&key(&f, "create"), &metadata("cancelled")).await.unwrap();
    let id = key(&f, "append");
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let append = f.storage.append_file_once(&id, &producer.session, 0, (b"bytes").to_vec());
        tokio::pin!(append);
        tokio::select! { biased; _ = &mut append => panic!("chunk must block"), _ = f.data.control.chunk_entered.notified() => {} }
    }
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_one();
    let appended = f.storage.append_file_once(&id, &producer.session, 0, (b"bytes").to_vec()).await.unwrap();
    assert_eq!(appended.length, 5);
    assert_eq!(*f.data.control.chunk_offsets.lock().unwrap(), vec![0]);
    f.finish().await;
}

#[tokio::test]
async fn concurrent_appends_exclusive() {
    let f = fixture().await;
    let producer = f.storage.create_file_once(&key(&f, "create"), &metadata("offset")).await.unwrap();
    let first = key(&f, "first");
    let second = key(&f, "second");
    let (first, second) = tokio::join!(
        f.storage.append_file_once(&first, &producer.session, 0, (b"first").to_vec()),
        f.storage.append_file_once(&second, &producer.session, 0, (b"second").to_vec())
    );
    assert_eq!(first.unwrap().length, 5);
    assert!(second.is_err());
    assert_eq!(f.data.staged.lock().unwrap()["offset"].1, b"first");
    f.finish().await;
}

#[tokio::test]
async fn integrity_failure_abandons() {
    let f = fixture().await;
    let mut metadata = metadata("integrity");
    metadata.expected_length = Some(6);
    let producer = f.storage.create_file_once(&key(&f, "create"), &metadata).await.unwrap();
    f.storage.append_file_once(&key(&f, "append"), &producer.session, 0, (b"bad").to_vec()).await.unwrap();
    let id = key(&f, "finish");
    assert!(f.storage.finish_file_once(&id, &producer.session).await.is_err());
    assert!(f.storage.finish_file_once(&id, &producer.session).await.is_err());
    let identity = StagedFile { history: producer.session.history.clone(), id: producer.session.id.clone() };
    assert_eq!(f.storage.file_progress(&identity).await.unwrap().phase, FilePhase::Abandoned);
    assert_eq!(f.data.staged.lock().unwrap()["integrity"].1, b"bad");
    f.finish().await;
}

#[tokio::test]
async fn receipt_awaits_flush() {
    let f = fixture().await;
    let through = f.storage.cursor_accepted();
    let producer = f.storage.create_file_once(&key(&f, "create"), &metadata("frontier")).await.unwrap();
    let id = key(&f, "finish");
    let reference = f.storage.finish_file_once(&id, &producer.session).await.unwrap();
    assert!(reference.position.sequence > through.sequence);
    f.storage.flush_until(&through).await.unwrap();
    assert!(!f.data.mutation_receipts.lock().unwrap().contains_key(&id));
    f.storage.flush().await.unwrap();
    assert!(f.data.mutation_receipts.lock().unwrap().contains_key(&id));
    f.finish().await;
}

#[tokio::test]
async fn unsaved_file_survives() {
    let f = fixture().await;
    let first = f.storage.write_file(metadata("first-finished"), (b"first").to_vec()).await.unwrap();
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    let recovered = super::recovery::restart(&f).await;
    let second = recovered.storage.write_file(metadata("second-finished"), (b"second").to_vec()).await.unwrap();
    assert_eq!(first.position, second.position);
    for (reference, expected) in [(first, b"first".as_slice()), (second, b"second".as_slice())] {
        let identity = StagedFile { history: reference.position.history, id: reference.metadata.id };
        let mut reader = recovered.storage.follow_production(&identity, 0).await.unwrap();
        let chunk = reader.next_batch(NonZeroUsize::new(32).unwrap()).await.unwrap().unwrap();
        assert_eq!(chunk.bytes(), expected);
        assert!(reader.next_batch(NonZeroUsize::MIN).await.unwrap().is_none());
    }
    recovered.finish().await;
    f.finish().await;
}

#[tokio::test]
async fn finish_reconciles_lost() {
    let f = fixture().await;
    let producer = f.storage.create_file_once(&key(&f, "create"), &metadata("lost-finish")).await.unwrap();
    f.storage.append_file_once(&key(&f, "append"), &producer.session, 0, (b"finished").to_vec()).await.unwrap();
    f.data.control.finish_failures.store(1, Ordering::SeqCst);
    let request = FinishFile { mutation: key(&f, "finish"), session: producer.session };
    let result = f.runtime.operations.call(request.clone()).await.unwrap();
    let replay = f.runtime.operations.call(request).await.unwrap();
    assert_eq!(result, replay);
    assert_eq!(f.data.control.finish_positions.lock().unwrap().as_slice(), &[result.position.sequence, result.position.sequence]);
    f.finish().await;
}

#[tokio::test]
async fn retries_never_multiply() {
    let f = Fixture::new(config()).await;
    harmos_runtime::storage::operations::register(
        &f.runtime.operations,
        "storage",
        crate::storage::operations::Lanes::Open,
        Arc::new(f.storage.clone()),
    )
    .unwrap();
    let request = CreateFile { mutation: key(&f, "create"), metadata: metadata("retry-budget") };
    f.data.control.create_failures.store(5, Ordering::SeqCst);
    assert!(f.runtime.operations.call(request.clone()).await.is_err());
    assert_eq!(f.data.control.create_failures.load(Ordering::SeqCst), 1);
    f.data.control.create_failures.store(0, Ordering::SeqCst);
    let producer = f.runtime.operations.call(request).await.unwrap();
    f.storage.abandon_file_once(&key(&f, "abandon"), &producer.session).await.unwrap();
    f.finish().await;
}
