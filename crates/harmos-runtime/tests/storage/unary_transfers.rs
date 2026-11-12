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
    let appended = super::transfer_plugin::client_call::<operations::AppendFileBytes>(&channel, append.clone()).await.unwrap();
    assert_eq!(super::transfer_plugin::client_call::<operations::AppendFileBytes>(&channel, append.clone()).await.unwrap(), appended);

    // Reconnect Through Context, Inspect Progress and Fence the Old Writer.
    let file = payload::StagedFile {
        id: "unary-file".into(),
        history: first_writer.as_ref().unwrap().history.clone(),
    };
    let status = super::transfer_plugin::call::<context::GetFileProduction>(
        calls,
        child::StorageGetFileProductionRequest { file: Some(file.clone()) },
    )
    .await
    .unwrap();
    assert_eq!(status.production.unwrap().length, 6);
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
        at: None,
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
    assert!(
        super::transfer_plugin::client_call::<operations::GetFileBytes>(
            &channel,
            client::StorageGetFileBytesRequest { max_bytes: 0, ..request.clone() }
        )
        .await
        .is_err()
    );
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
