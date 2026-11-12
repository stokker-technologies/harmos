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
use std::sync::atomic::Ordering;

// Third Party Imports

// Local Imports
use super::metadata;
use super::production::{fixture, key};
use harmos_runtime::Identifier;
use harmos_runtime::storage::StagedFile;

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn mutation_receipts_survive_replays_lost_responses_and_cancellation() {
    // Start
    let f = fixture().await;

    // Replay Successful Mutations Without Duplicating Effects

    let create = harmos_protocol::storage::CreateFile {
        mutation: Some(key(&f, "create").into()),
        metadata: Some(metadata("shared").into()),
    };
    let producer = f.runtime.operations.call_protobuf::<harmos_operations::storage::CreateFile>(create.clone()).await.unwrap();
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::CreateFile>(create.clone()).await.unwrap(), producer);
    let first = harmos_protocol::storage::AppendFile {
        mutation: Some(key(&f, "first").into()),
        session: producer.session.clone(),
        offset: 0,
        bytes: b"one".to_vec(),
    };
    let appended = f.runtime.operations.call_protobuf::<harmos_operations::storage::AppendFile>(first.clone()).await.unwrap();
    assert_eq!(appended.length, 3);
    let second = harmos_protocol::storage::AppendFile {
        mutation: Some(key(&f, "second").into()),
        offset: 3,
        bytes: b"two".to_vec(),
        ..first.clone()
    };
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::AppendFile>(second).await.unwrap().length, 6);
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::AppendFile>(first.clone()).await.unwrap(), appended);
    let conflict = harmos_protocol::storage::AppendFile { bytes: b"bad".to_vec(), ..first.clone() };
    assert!(f.runtime.operations.call_protobuf::<harmos_operations::storage::AppendFile>(conflict).await.is_err());
    let wrong_offset = harmos_protocol::storage::AppendFile {
        mutation: Some(key(&f, "wrong-offset").into()),
        ..first
    };
    assert!(f.runtime.operations.call_protobuf::<harmos_operations::storage::AppendFile>(wrong_offset).await.is_err());
    let finish = harmos_protocol::storage::FinishFile {
        mutation: Some(key(&f, "finish").into()),
        session: producer.session.clone(),
    };
    let reference = f.runtime.operations.call_protobuf::<harmos_operations::storage::FinishFile>(finish.clone()).await.unwrap();
    assert_eq!(reference.length, 6);
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::FinishFile>(finish.clone()).await.unwrap(), reference);
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::CreateFile>(create).await.unwrap(), producer);
    let identity = StagedFile {
        history: producer.session.as_ref().unwrap().history.clone().unwrap().try_into().unwrap(),
        id: Identifier::new(producer.session.as_ref().unwrap().id.clone()).unwrap(),
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
        .call_protobuf::<harmos_operations::storage::CreateFile>(harmos_protocol::storage::CreateFile {
            mutation: Some(key(&f, "lost-create").into()),
            metadata: Some(metadata("lost").into()),
        })
        .await
        .unwrap();
    assert_eq!(producer.session.as_ref().unwrap().generation, 1);
    assert_eq!(f.data.control.create_failures.load(Ordering::SeqCst), 0);
    f.data.control.chunk_failures.store(3, Ordering::SeqCst);
    let append = harmos_protocol::storage::AppendFile {
        mutation: Some(key(&f, "lost-append").into()),
        session: producer.session.clone(),
        offset: 0,
        bytes: b"partial".to_vec(),
    };
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::AppendFile>(append.clone()).await.unwrap().length, 7);
    assert_eq!(f.data.control.chunk_offsets.lock().unwrap()[offset_start..], vec![0, 0, 0, 0]);
    f.data.control.abandon_failures.store(3, Ordering::SeqCst);
    let abandon = harmos_protocol::storage::AbandonFile {
        mutation: Some(key(&f, "lost-abandon").into()),
        session: producer.session.clone(),
    };
    let abandoned = f.runtime.operations.call_protobuf::<harmos_operations::storage::AbandonFile>(abandon.clone()).await.unwrap();
    assert_eq!(abandoned.phase, harmos_protocol::storage::FilePhase::Abandoned as i32);
    assert_eq!(abandoned.length, 7);
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::AbandonFile>(abandon).await.unwrap(), abandoned);
    assert_eq!(f.runtime.operations.call_protobuf::<harmos_operations::storage::AppendFile>(append).await.unwrap().length, 7);
    assert_eq!(f.data.staged.lock().unwrap()["lost"].1, b"partial");
    f.data.control.delete_failures.store(3, Ordering::SeqCst);
    let delete = harmos_protocol::storage::DeleteFile {
        mutation: Some(key(&f, "lost-delete").into()),
        file: Some(harmos_protocol::storage::StagedFile {
            history: producer.session.as_ref().unwrap().history.clone(),
            id: producer.session.as_ref().unwrap().id.clone(),
        }),
    };
    let deleted = f.runtime.operations.call_protobuf::<harmos_operations::storage::DeleteFile>(delete.clone()).await.unwrap();
    assert_eq!(Some(deleted), delete.file);
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
