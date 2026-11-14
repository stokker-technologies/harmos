/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::metadata;
use super::production::{fixture, key};
use harmos_runtime::Identifier;
use harmos_runtime::storage::StagedFile;
use sha2::Digest;

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn mutation_receipts_survive_replays_lost_responses_and_cancellation() {
    // Start
    let f = fixture().await;

    // Replay Successful Mutations Without Duplicating Effects

    let create = harmos_operations::storage::CreateFileProductionRequest {
        mutation: Some(key(&f, "create").into()),
        metadata: Some(metadata("shared").into()),
    };
    let created = f.runtime.operations.call_native::<harmos_operations::storage::CreateFileProduction>(create.clone()).await.unwrap();
    let producer = created.production.clone().unwrap();
    assert_eq!(
        f.runtime.operations.call_native::<harmos_operations::storage::CreateFileProduction>(create.clone()).await.unwrap(),
        created
    );
    let first = harmos_operations::storage::AppendFileBytesRequest {
        mutation: Some(key(&f, "first").into()),
        session: Some(producer.session.clone()),
        offset: 0,
        data: b"one".to_vec(),
    };
    let appended_reply = f.runtime.operations.call_native::<harmos_operations::storage::AppendFileBytes>(first.clone()).await.unwrap();
    let appended = appended_reply.production.clone().unwrap();
    assert_eq!(appended.length, 3);
    let second = harmos_operations::storage::AppendFileBytesRequest {
        mutation: Some(key(&f, "second").into()),
        offset: 3,
        data: b"two".to_vec(),
        ..first.clone()
    };
    assert_eq!(
        f.runtime.operations.call_native::<harmos_operations::storage::AppendFileBytes>(second).await.unwrap().production.unwrap().length,
        6
    );
    assert_eq!(
        f.runtime.operations.call_native::<harmos_operations::storage::AppendFileBytes>(first.clone()).await.unwrap(),
        appended_reply
    );
    let conflict = harmos_operations::storage::AppendFileBytesRequest { data: b"bad".to_vec(), ..first.clone() };
    assert!(f.runtime.operations.call_native::<harmos_operations::storage::AppendFileBytes>(conflict).await.is_err());
    let identical_prefix = harmos_operations::storage::AppendFileBytesRequest {
        mutation: Some(key(&f, "wrong-offset").into()),
        ..first
    };
    assert_eq!(
        f.runtime
            .operations
            .call_native::<harmos_operations::storage::AppendFileBytes>(identical_prefix)
            .await
            .unwrap()
            .production
            .unwrap()
            .length,
        6
    );
    let finish = harmos_operations::storage::PublishFileProductionRequest {
        mutation: Some(key(&f, "finish").into()),
        length: 6,
        sha256: sha2::Sha256::digest(b"onetwo").to_vec(),
        session: Some(producer.session.clone()),
    };
    let published = f.runtime.operations.call_native::<harmos_operations::storage::PublishFileProduction>(finish.clone()).await.unwrap();
    let reference = published.file.clone().unwrap();
    assert_eq!(reference.length, 6);
    assert_eq!(
        f.runtime.operations.call_native::<harmos_operations::storage::PublishFileProduction>(finish.clone()).await.unwrap(),
        published
    );
    assert_eq!(f.runtime.operations.call_native::<harmos_operations::storage::CreateFileProduction>(create).await.unwrap(), created);
    let identity = StagedFile {
        history: producer.session.history.clone(),
        id: producer.session.id.clone(),
    };
    let mut reader = f.storage.follow_production(&identity, 0).await.unwrap();
    assert_eq!(reader.next_batch(NonZeroUsize::new(16).unwrap()).await.unwrap().unwrap().bytes(), b"onetwo");
    assert!(reader.next_batch(NonZeroUsize::MIN).await.unwrap().is_none());
    drop(reader);

    // Recover Lost Responses Within The Same Mutation Receipts

    let offset_start = f.data.control.chunk_offsets.lock().unwrap().len();

    f.data.control.create_failures.store(3, Ordering::SeqCst);
    let producer = f
        .runtime
        .operations
        .call_native::<harmos_operations::storage::CreateFileProduction>(harmos_operations::storage::CreateFileProductionRequest {
            mutation: Some(key(&f, "lost-create").into()),
            metadata: Some(metadata("lost").into()),
        })
        .await
        .unwrap()
        .production
        .unwrap();
    assert_eq!(producer.session.generation, 1);
    assert_eq!(f.data.control.create_failures.load(Ordering::SeqCst), 0);
    f.data.control.chunk_failures.store(3, Ordering::SeqCst);
    let append = harmos_operations::storage::AppendFileBytesRequest {
        mutation: Some(key(&f, "lost-append").into()),
        session: Some(producer.session.clone()),
        offset: 0,
        data: b"partial".to_vec(),
    };
    assert_eq!(
        f.runtime
            .operations
            .call_native::<harmos_operations::storage::AppendFileBytes>(append.clone())
            .await
            .unwrap()
            .production
            .unwrap()
            .length,
        7
    );
    assert_eq!(f.data.control.chunk_offsets.lock().unwrap()[offset_start..], vec![0, 0, 0, 0]);
    f.data.control.abandon_failures.store(3, Ordering::SeqCst);
    let abandon = harmos_operations::storage::DeleteFileProductionRequest {
        mutation: Some(key(&f, "lost-abandon").into()),
        session: Some(producer.session.clone()),
    };
    let abandoned_reply =
        f.runtime.operations.call_native::<harmos_operations::storage::DeleteFileProduction>(abandon.clone()).await.unwrap();
    let abandoned = abandoned_reply.production.clone().unwrap();
    assert_eq!(abandoned.phase, harmos_storage::FilePhase::Abandoned);
    assert_eq!(abandoned.length, 7);
    assert_eq!(
        f.runtime.operations.call_native::<harmos_operations::storage::DeleteFileProduction>(abandon).await.unwrap(),
        abandoned_reply
    );
    assert_eq!(
        f.runtime.operations.call_native::<harmos_operations::storage::AppendFileBytes>(append).await.unwrap().production.unwrap().length,
        7
    );
    assert_eq!(f.data.staged.lock().unwrap()["lost"].1, b"partial");
    f.data.control.delete_failures.store(3, Ordering::SeqCst);
    let delete = harmos_operations::storage::DeleteFileRequest {
        mutation: Some(key(&f, "lost-delete").into()),
        file: Some(harmos_storage::StagedFile {
            history: producer.session.history.clone(),
            id: producer.session.id.clone(),
        }),
    };
    let deleted = f.runtime.operations.call_native::<harmos_operations::storage::DeleteFile>(delete.clone()).await.unwrap();
    assert_eq!(deleted.file, delete.file);
    assert!(deleted.receipt.unwrap().completed);
    assert_eq!(f.data.control.delete_failures.load(Ordering::SeqCst), 0);

    // Cancel An Append And Reattach To Its In-Flight Work

    let offset_start = f.data.control.chunk_offsets.lock().unwrap().len();

    let producer = f.storage.create_file_once(&key(&f, "cancelled-create"), &metadata("cancelled")).await.unwrap();
    let id = key(&f, "cancelled-append");
    f.data.control.block_chunk.store(true, Ordering::SeqCst);
    {
        let append = f.storage.append_file_once(&id, &producer.session, 0, (b"bytes").to_vec());
        tokio::pin!(append);
        assert!(crate::support::events::until(append.as_mut(), f.data.control.chunk_entered.notified()).await.is_ok());
    }
    f.data.control.block_chunk.store(false, Ordering::SeqCst);
    f.data.control.chunk_release.notify_one();
    let appended = f.storage.append_file_once(&id, &producer.session, 0, (b"bytes").to_vec()).await.unwrap();
    assert_eq!(appended.length, 5);
    assert_eq!(f.data.control.chunk_offsets.lock().unwrap()[offset_start..], vec![0]);

    // Stop
    let operations = f.runtime.operations.clone();
    f.finish().await;
    assert!(operations.resident(&harmos_runtime::operations::resident::Key::root("harmos-storage")).is_none());
}
