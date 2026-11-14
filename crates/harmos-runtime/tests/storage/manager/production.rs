/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use harmos_operations::storage::{CreateFileProductionRequest, PublishFileProductionRequest};

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::{Fixture, config, metadata};
use harmos_runtime::Identifier;
use harmos_runtime::storage::{FilePhase, MutationId, StagedFile};
use sha2::Digest;

/* === Private Functions === */

pub(super) async fn fixture() -> Fixture {
    let f = Fixture::new(config()).await;
    crate::support::app::register(&f.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    f
}
pub(super) fn key(f: &Fixture, key: &str) -> MutationId {
    MutationId {
        window: f.storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}

/* === Tests === */

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
    let identity = StagedFile {
        history: producer.session.history.clone(),
        id: producer.session.id.clone(),
    };
    assert_eq!(f.storage.get_file_production(&identity).await.unwrap().phase, FilePhase::Abandoned);
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
    assert_eq!(f.storage.get_saved_position().sequence, 0);
    let recovered = super::recovery::restart(&f).await;
    let second = recovered.storage.write_file(metadata("second-finished"), (b"second").to_vec()).await.unwrap();
    assert_eq!(first.position, second.position);
    for (reference, expected) in [(first, b"first".as_slice()), (second, b"second".as_slice())] {
        let identity = StagedFile {
            history: reference.position.history,
            id: reference.metadata.id,
        };
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
    let request = PublishFileProductionRequest {
        mutation: Some(key(&f, "finish")),
        length: 8,
        sha256: sha2::Sha256::digest(b"finished").to_vec(),
        session: Some(producer.session),
    };
    let result = f.runtime.operations.call_native::<harmos_operations::storage::PublishFileProduction>(request.clone()).await.unwrap();
    let replay = f.runtime.operations.call_native::<harmos_operations::storage::PublishFileProduction>(request).await.unwrap();
    assert_eq!(result, replay);
    assert_eq!(
        f.data.control.finish_positions.lock().unwrap().as_slice(),
        &[result.file.as_ref().unwrap().position.sequence, result.file.as_ref().unwrap().position.sequence]
    );
    f.finish().await;
}

#[tokio::test]
async fn retries_never_multiply() {
    let f = Fixture::new(config()).await;
    crate::support::app::register(&f.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let request = CreateFileProductionRequest {
        mutation: Some(key(&f, "create")),
        metadata: Some(metadata("retry-budget")),
    };
    f.data.control.create_failures.store(5, Ordering::SeqCst);
    assert!(f.runtime.operations.call_native::<harmos_operations::storage::CreateFileProduction>(request.clone()).await.is_err());
    assert_eq!(f.data.control.create_failures.load(Ordering::SeqCst), 1);
    f.data.control.create_failures.store(0, Ordering::SeqCst);
    let producer =
        f.runtime.operations.call_native::<harmos_operations::storage::CreateFileProduction>(request).await.unwrap().production.unwrap();
    f.storage.abandon_file_once(&key(&f, "abandon"), &producer.session).await.unwrap();
    f.finish().await;
}
