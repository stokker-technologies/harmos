/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resume Unary Transfers Across Audience Faces

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as payload, v1 as client};
use sha2::{Digest, Sha256};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn resume_fence_publish_and_range_across_faces() {
    // Start the Shared Owner.
    let fixture = super::Fixture::new(super::config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let settings = harmos_runtime::plugin::grpc::PlacementSettings {
        connect_timeout: std::time::Duration::from_secs(30),
        reconnect_delay: std::time::Duration::from_secs(1),
        stop_grace: std::time::Duration::from_secs(30),
        stream_window: std::num::NonZeroUsize::new(64).unwrap(),
    };
    let attached =
        fixture.runtime.attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()), settings).await.unwrap();
    fixture.runtime.operations.register_authorizer(std::sync::Arc::new(super::transfer_plugin::AllowTransfers)).unwrap();
    let mut incompatible = harmos_runtime::grpc::Settings::default();
    incompatible.operations.decode.message_bytes = std::num::NonZeroUsize::new(1024).unwrap();
    assert!(matches!(
        fixture.runtime.grpc().attach(incompatible).await,
        Err(harmos_runtime::Error::InvalidInput {
            field: "GrpcSettings.operations.decode.message_bytes",
            ..
        })
    ));
    let listener = fixture.runtime.grpc().attach(harmos_runtime::grpc::Settings::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let calls = &fixture.runtime.operations;
    let window = fixture.storage.mutation_window().unwrap().id.to_string();
    let mutation = |key: &str| Some(payload::MutationId { window: window.clone(), key: key.into() });
    let content = b"first-second";
    let digest = Sha256::digest(content).to_vec();

    // Create Through the Client Face, Retrying a Lost Reply.
    let create = client::StorageCreateFileProductionRequest {
        metadata: Some(payload::FileMetadata {
            id: "unary-file".into(),
            media_type: "application/octet-stream".into(),
            expected_length: Some(content.len() as u64),
            expected_sha256: Some(digest.clone()),
            ..Default::default()
        }),
        mutation: mutation("create"),
    };
    fixture.data.control.unsupported_sessions.store(true, std::sync::atomic::Ordering::Release);
    assert!(super::transfer_plugin::client_call::<operations::CreateFileProduction>(&channel, create.clone()).await.is_err());
    fixture.data.control.unsupported_sessions.store(false, std::sync::atomic::Ordering::Release);
    let denied = super::transfer_plugin::client_call::<operations::CreateFileProduction>(
        &channel,
        client::StorageCreateFileProductionRequest {
            metadata: Some(payload::FileMetadata {
                id: "ungranted-file".into(),
                ..create.metadata.clone().unwrap()
            }),
            ..create.clone()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(denied.code(), tonic::Code::PermissionDenied);
    let created = super::transfer_plugin::client_call::<operations::CreateFileProduction>(&channel, create.clone()).await.unwrap();
    assert_eq!(super::transfer_plugin::client_call::<operations::CreateFileProduction>(&channel, create).await.unwrap(), created);
    let first_writer = created.production.unwrap().session;
    let append = client::StorageAppendFileBytesRequest {
        session: first_writer.clone(),
        offset: 0,
        data: b"first-".to_vec(),
        mutation: mutation("first"),
    };
    let file = payload::StagedFile {
        id: "unary-file".into(),
        history: first_writer.as_ref().unwrap().history.clone(),
    };
    fixture.data.control.block_chunk.store(true, std::sync::atomic::Ordering::Release);
    {
        let pending = super::transfer_plugin::client_call::<operations::AppendFileBytes>(&channel, append.clone());
        tokio::pin!(pending);
        crate::support::events::until(pending.as_mut(), fixture.data.control.chunk_entered.notified()).await.unwrap();
        let status = super::transfer_plugin::call::<context::GetFileProduction>(
            calls,
            child::StorageGetFileProductionRequest { file: Some(file.clone()), at: None },
        )
        .await
        .unwrap();
        assert_eq!(status.accepted_length, 6);
        assert_eq!(status.durable_length, 0);
        // Lose the Client Reply While Its Admitted Storage Work Remains Owned.
    }
    fixture.data.control.block_chunk.store(false, std::sync::atomic::Ordering::Release);
    fixture.data.control.chunk_release.notify_one();
    let appended = super::transfer_plugin::client_call::<operations::AppendFileBytes>(&channel, append.clone()).await.unwrap();
    assert_eq!(super::transfer_plugin::client_call::<operations::AppendFileBytes>(&channel, append.clone()).await.unwrap(), appended);

    // Persist the Recoverable Prefix; a Lost Checkpoint Reply Cannot Acknowledge Durability.
    let save = fixture.storage.capture_files(None).unwrap();
    fixture.data.control.checkpoint_failures.store(1, std::sync::atomic::Ordering::Release);
    assert!(fixture.storage.persist_files(&save).await.is_err());
    let status = super::transfer_plugin::call::<context::GetFileProduction>(
        calls,
        child::StorageGetFileProductionRequest { file: Some(file.clone()), at: None },
    )
    .await
    .unwrap();
    assert_eq!(status.accepted_length, 6);
    assert_eq!(status.durable_length, 0);
    let pending_cut = status.resolved_at.clone();
    fixture.storage.persist_files(&save).await.unwrap();
    let previous = super::transfer_plugin::client_call::<operations::GetFileProduction>(
        &channel,
        client::StorageGetFileProductionRequest {
            file: Some(file.clone()),
            at: pending_cut.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(previous.accepted_length, 6);
    assert_eq!(previous.durable_length, 0);
    assert_eq!(previous.resolved_at, pending_cut);

    // Reconnect Through Context, Inspect Progress and Fence the Old Writer.
    let status = super::transfer_plugin::call::<context::GetFileProduction>(
        calls,
        child::StorageGetFileProductionRequest { file: Some(file.clone()), at: None },
    )
    .await
    .unwrap();
    assert_eq!(status.production.unwrap().length, 6);
    assert_eq!(status.accepted_length, 6);
    assert_eq!(status.durable_length, 6);
    assert_eq!(status.writer_generation, first_writer.as_ref().unwrap().generation);
    let resumed = super::transfer_plugin::call::<context::CreateFileWriter>(
        calls,
        child::StorageCreateFileWriterRequest {
            session: first_writer.clone(),
            mutation: mutation("resume"),
        },
    )
    .await
    .unwrap()
    .production
    .unwrap();
    assert!(resumed.session.as_ref().unwrap().generation > first_writer.as_ref().unwrap().generation);
    assert!(
        super::transfer_plugin::client_call::<operations::AppendFileBytes>(
            &channel,
            client::StorageAppendFileBytesRequest {
                mutation: mutation("stale"),
                offset: 6,
                data: b"second".to_vec(),
                ..append.clone()
            }
        )
        .await
        .is_err()
    );

    // Replay an Accepted Prefix Under a New Identity; Reject Conflicting Bytes and Gaps.
    let replay = child::StorageAppendFileBytesRequest {
        session: resumed.session.clone(),
        offset: 0,
        data: b"first-".to_vec(),
        mutation: mutation("replayed-prefix"),
    };
    assert_eq!(
        super::transfer_plugin::call::<context::AppendFileBytes>(calls, replay.clone()).await.unwrap().production.unwrap().length,
        6
    );
    assert!(
        super::transfer_plugin::call::<context::AppendFileBytes>(
            calls,
            child::StorageAppendFileBytesRequest {
                data: b"other-".to_vec(),
                mutation: mutation("conflicting-prefix"),
                ..replay.clone()
            }
        )
        .await
        .is_err()
    );
    assert!(
        super::transfer_plugin::call::<context::AppendFileBytes>(
            calls,
            child::StorageAppendFileBytesRequest {
                offset: 7,
                mutation: mutation("gap"),
                ..replay.clone()
            }
        )
        .await
        .is_err()
    );
    let completed = super::transfer_plugin::call::<context::AppendFileBytes>(
        calls,
        child::StorageAppendFileBytesRequest {
            offset: 6,
            data: b"second".to_vec(),
            mutation: mutation("second"),
            ..replay
        },
    )
    .await
    .unwrap();
    assert_eq!(completed.production.unwrap().length, content.len() as u64);

    // Publish With Explicit Integrity and Retain Its Durable Position.
    let publish = child::StoragePublishFileProductionRequest {
        session: resumed.session,
        length: content.len() as u64,
        sha256: digest.clone(),
        mutation: mutation("publish"),
    };
    let published = super::transfer_plugin::call::<context::PublishFileProduction>(calls, publish.clone()).await.unwrap();
    assert_eq!(super::transfer_plugin::call::<context::PublishFileProduction>(calls, publish).await.unwrap(), published);
    let durable_at = published.receipt.unwrap().durable_at.unwrap();
    let reference = published.file.unwrap();
    assert_eq!(reference.length, content.len() as u64);

    // Read Exact Unary Ranges on Both Faces at One Durable Cut.
    let request = client::StorageGetFileBytesRequest {
        file: Some(reference.clone()),
        offset: 0,
        max_bytes: 5,
        at: Some(client::ReadCut {
            positions: vec![durable_at.clone()],
            revisions: vec![],
        }),
    };
    let first = super::transfer_plugin::client_call::<operations::GetFileBytes>(&channel, request.clone()).await.unwrap();
    assert_eq!(first.data, content[..5]);
    assert_eq!(first.sha256, digest);
    assert_eq!(first.total_length, content.len() as u64);
    assert_eq!(first.resolved_at.as_ref().unwrap().positions, vec![durable_at]);
    let next = super::transfer_plugin::call::<context::GetFileBytes>(
        calls,
        child::StorageGetFileBytesRequest {
            file: Some(reference.clone()),
            offset: 5,
            max_bytes: 100,
            at: first.resolved_at.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(next.data, content[5..]);
    assert_eq!(next.sha256, first.sha256);
    assert_eq!(next.resolved_at, first.resolved_at);
    assert!(
        super::transfer_plugin::client_call::<operations::GetFileBytes>(
            &channel,
            client::StorageGetFileBytesRequest {
                offset: content.len() as u64 + 1,
                ..request.clone()
            }
        )
        .await
        .is_err()
    );
    let default_range = super::transfer_plugin::client_call::<operations::GetFileBytes>(
        &channel,
        client::StorageGetFileBytesRequest { max_bytes: 0, ..request.clone() },
    )
    .await
    .unwrap();
    assert_eq!(default_range.data, content);
    assert_eq!(default_range.resolved_at, first.resolved_at);
    assert!(
        super::transfer_plugin::client_call::<operations::GetFileBytes>(
            &channel,
            client::StorageGetFileBytesRequest { offset: content.len() as u64, ..request }
        )
        .await
        .unwrap()
        .data
        .is_empty()
    );

    // Enforce the Full Unary Response Ceiling and Verify Whole-Content Corruption.
    fixture.data.control.corrupt_file.store(true, std::sync::atomic::Ordering::Release);
    assert!(
        super::transfer_plugin::client_call::<operations::GetFileBytes>(
            &channel,
            client::StorageGetFileBytesRequest {
                file: Some(reference.clone()),
                offset: 0,
                max_bytes: u64::MAX,
                at: None,
            }
        )
        .await
        .is_err()
    );
    fixture.data.control.corrupt_file.store(false, std::sync::atomic::Ordering::Release);
    let bound = fixture.storage.settings().transfer_message_bytes().get();
    let bytes = vec![0x5a; bound + 1000];
    let digest = Sha256::digest(&bytes).to_vec();
    let production = super::transfer_plugin::client_call::<operations::CreateFileProduction>(
        &channel,
        client::StorageCreateFileProductionRequest {
            metadata: Some(payload::FileMetadata {
                id: "bounded-unary".into(),
                media_type: "application/octet-stream".into(),
                ..Default::default()
            }),
            mutation: mutation("bounded-create"),
        },
    )
    .await
    .unwrap()
    .production
    .unwrap();
    let chunk_bytes = fixture.storage.settings().transfer_chunk_bytes().get();
    for (index, chunk) in bytes.chunks(chunk_bytes).enumerate() {
        super::transfer_plugin::client_call::<operations::AppendFileBytes>(
            &channel,
            client::StorageAppendFileBytesRequest {
                session: production.session.clone(),
                offset: (index * chunk_bytes) as u64,
                data: chunk.to_vec(),
                mutation: mutation(&format!("bounded-append-{index}")),
            },
        )
        .await
        .unwrap();
    }
    let published = super::transfer_plugin::client_call::<operations::PublishFileProduction>(
        &channel,
        client::StoragePublishFileProductionRequest {
            session: production.session,
            length: bytes.len() as u64,
            sha256: digest,
            mutation: mutation("bounded-publish"),
        },
    )
    .await
    .unwrap();
    let range = super::transfer_plugin::client_call::<operations::GetFileBytes>(
        &channel,
        client::StorageGetFileBytesRequest {
            file: published.file,
            offset: 0,
            max_bytes: u64::MAX,
            at: None,
        },
    )
    .await
    .unwrap();
    assert!(prost::Message::encoded_len(&range) <= bound);
    assert_eq!(range.data, bytes[..chunk_bytes]);

    // Abandon Another Production Without Publishing It.
    let created = super::transfer_plugin::call::<context::CreateFileProduction>(
        calls,
        child::StorageCreateFileProductionRequest {
            metadata: Some(payload::FileMetadata {
                id: "abandoned-unary".into(),
                media_type: "application/octet-stream".into(),
                ..Default::default()
            }),
            mutation: mutation("create-abandoned"),
        },
    )
    .await
    .unwrap();
    let abandoned = super::transfer_plugin::client_call::<operations::DeleteFileProduction>(
        &channel,
        client::StorageDeleteFileProductionRequest {
            session: created.production.unwrap().session,
            mutation: mutation("abandon"),
        },
    )
    .await
    .unwrap();
    assert_eq!(abandoned.production.unwrap().phase, payload::FilePhase::Abandoned as i32);
    // Refuse a Wrong Publication Digest Without Exposing Content.
    let invalid = super::transfer_plugin::client_call::<operations::CreateFileProduction>(
        &channel,
        client::StorageCreateFileProductionRequest {
            metadata: Some(payload::FileMetadata {
                id: "invalid-unary".into(),
                media_type: "application/octet-stream".into(),
                ..Default::default()
            }),
            mutation: mutation("invalid-create"),
        },
    )
    .await
    .unwrap()
    .production
    .unwrap();
    super::transfer_plugin::client_call::<operations::AppendFileBytes>(
        &channel,
        client::StorageAppendFileBytesRequest {
            session: invalid.session.clone(),
            offset: 0,
            data: b"data".to_vec(),
            mutation: mutation("invalid-append"),
        },
    )
    .await
    .unwrap();
    assert!(
        super::transfer_plugin::client_call::<operations::PublishFileProduction>(
            &channel,
            client::StoragePublishFileProductionRequest {
                session: invalid.session,
                length: 4,
                sha256: vec![0; 32],
                mutation: mutation("invalid-publish")
            }
        )
        .await
        .is_err()
    );
    attached.stop("transfer story complete").await.unwrap();
    plugin.stop().await;
    drop(channel);
    listener.detach().await.unwrap();
    fixture.finish().await;
}
