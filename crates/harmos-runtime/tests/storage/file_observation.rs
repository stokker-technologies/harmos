/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Observe File Content, Production Prefixes And Directory Entries On Both Faces

use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as wire, v1};
use prost::Message;
use std::{num::NonZeroUsize, sync::Arc};

#[tokio::test]
async fn distinct_file_observations_preserve_bounded_frames_and_terminal_progress() {
    let mut settings = super::config();
    settings.response.batch_bytes = NonZeroUsize::new(4096).unwrap();
    let ceiling = settings.response.batch_bytes.get();
    let fixture = super::Fixture::new(settings).await;
    crate::support::app::register(&fixture.runtime, operations::storage_catalog(), Default::default()).await.unwrap();
    fixture.runtime.operations.register_authorizer(Arc::new(super::transfer_plugin::AllowTransfers)).unwrap();
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let attached = fixture
        .runtime
        .attach_grpc(
            harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()),
            harmos_runtime::plugin::grpc::PlacementSettings {
                connect_timeout: std::time::Duration::from_secs(30),
                reconnect_delay: std::time::Duration::from_secs(1),
                stop_grace: std::time::Duration::from_secs(30),
                stream_window: NonZeroUsize::new(64).unwrap(),
            },
        )
        .await
        .unwrap();
    let mut producer = fixture
        .storage
        .produce_file(harmos_storage::FileMetadata {
            id: harmos_storage::Identifier::new("unary-file").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        })
        .await
        .unwrap();
    let bytes: Vec<_> = (0..16391).map(|offset| (offset % 251) as u8).collect();
    producer.write_all(bytes.clone()).await.unwrap();
    let session = producer.session().unwrap();
    let staged = wire::StagedFile {
        history: Some(session.history.clone().into()),
        id: session.id.to_string(),
    };
    let progress = super::transfer_plugin::call::<context::WatchFileProduction>(
        &fixture.runtime.operations,
        child::StorageWatchFileProductionRequest { file: Some(staged), offset: 0 },
    )
    .await
    .unwrap();
    let Some(child::storage_watch_file_production_response::Event::Data(prefix)) = progress.event else {
        panic!("expected retained production bytes");
    };
    assert_eq!(prefix.offset, 0);
    assert_eq!(prefix.bytes, bytes[..prefix.bytes.len()]);
    let file = producer.finish().await.unwrap();
    fixture.storage.flush().await.unwrap();
    let source = wire::FileSource {
        source: Some(wire::file_source::Source::Stored(file.into())),
    };
    let first = super::transfer_plugin::call::<context::WatchFileBytes>(
        &fixture.runtime.operations,
        child::StorageWatchFileBytesRequest {
            source: Some(source.clone()),
            start: Some(wire::FollowStart {
                start: Some(wire::follow_start::Start::Beginning(wire::Empty {})),
            }),
        },
    )
    .await
    .unwrap();
    assert!(first.encoded_len() <= ceiling);
    let Some(wire::file_event::Event::Data(batch)) = first.event.unwrap().event else {
        panic!("expected immutable content bytes");
    };
    assert_eq!(batch.bytes, bytes[..batch.bytes.len()]);

    let listener = fixture.runtime.grpc().attach(Default::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let mut grpc = tonic::client::Grpc::new(channel.clone());
    grpc.ready().await.unwrap();
    let mut stream: tonic::Streaming<v1::StorageWatchFileBytesResponse> = grpc
        .server_streaming(
            tonic::Request::new(v1::StorageWatchFileBytesRequest {
                source: Some(source),
                start: Some(wire::FollowStart {
                    start: Some(wire::follow_start::Start::Beginning(wire::Empty {})),
                }),
            }),
            tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.v1.Storage/WatchFileBytes"),
            tonic_prost::ProstCodec::default(),
        )
        .await
        .unwrap()
        .into_inner();
    let mut received = Vec::new();
    let mut completion = None;
    while let Some(frame) = stream.message().await.unwrap() {
        assert!(frame.encoded_len() <= ceiling);
        if let Some(event) = frame.event {
            let Some(wire::file_event::Event::Data(batch)) = event.event else {
                panic!("immutable content only emits data");
            };
            assert_eq!(batch.offset, received.len() as u64);
            received.extend(batch.bytes);
        }
        if frame.completion.is_some() {
            completion = frame.completion;
        }
    }
    assert_eq!(received, bytes);
    assert_eq!(completion.unwrap().bytes, bytes.len() as u64);
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("entry.bin"), b"entry").unwrap();
    let directory = super::transfer_plugin::call::<context::WatchDirectoryEntries>(
        &fixture.runtime.operations,
        child::StorageWatchDirectoryEntriesRequest {
            selection: Some(wire::DirectorySelection {
                root: root.path().to_str().unwrap().into(),
                recursive: false,
                rules: vec![wire::DirectoryRule { pattern: "**".into(), include: true }],
            }),
            include_existing: true,
        },
    )
    .await
    .unwrap();
    let Some(wire::directory_event::Event::Existing(entry)) = directory.event.unwrap().event else {
        panic!("expected the observed directory entry");
    };
    assert!(entry.path.ends_with("entry.bin"));
    assert_eq!(entry.length, 5);
    attached.stop("file observation journey complete").await.unwrap();
    plugin.stop().await;
    drop(grpc);
    drop(channel);
    listener.detach().await.unwrap();
    fixture.finish().await;
}
