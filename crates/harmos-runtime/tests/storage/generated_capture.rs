/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Capture a Local File and Follow Its Stored Contents.

/* === Tests === */

#[tokio::test]
async fn capture_then_follow_stored_contents() {
    use harmos_operations::storage::{CaptureFile, WatchFileBytes};
    use harmos_protocol::{storage as wire, v1};
    use harmos_runtime::operations::{StreamSettings, WireStreamLimits};
    use prost::Message;
    use std::num::NonZeroUsize;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.bin");
    std::fs::write(&source, b"captured-content").unwrap();
    let fixture = super::Fixture::new(super::config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let reference = fixture
        .runtime
        .operations
        .call_protobuf::<CaptureFile>(wire::CaptureFile {
            source: Some(wire::FileSource {
                source: Some(wire::file_source::Source::Local(source.to_str().unwrap().into())),
            }),
            range: Some(wire::ByteRange { offset: 0, limit: None }),
            metadata: Some(wire::FileMetadata {
                id: "captured-file".into(),
                media_type: "application/octet-stream".into(),
                ..Default::default()
            }),
            consistency: wire::CaptureConsistency::Stable as i32,
        })
        .await
        .unwrap();
    assert_eq!(reference.length, 16);
    fixture.storage.flush().await.unwrap();
    let limit = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let (invocation, streams) = fixture
        .runtime
        .operations
        .submit_protobuf::<WatchFileBytes>(
            v1::StorageWatchFileBytesRequest {
                source: Some(wire::FileSource {
                    source: Some(wire::file_source::Source::Stored(reference)),
                }),
                start: Some(wire::FollowStart {
                    start: Some(wire::follow_start::Start::Beginning(wire::Empty {})),
                }),
            },
            Some(WireStreamLimits { input: limit, output: limit }),
        )
        .unwrap();
    let streams = streams.unwrap();
    let mut contents = Vec::new();
    let mut completed = None;
    while let Some(bytes) = streams.receive_current().await.unwrap() {
        let response = v1::StorageWatchFileBytesResponse::decode(bytes.as_slice()).unwrap();
        if let Some(completion) = response.completion {
            completed = Some(completion);
            continue;
        }
        let event = response.event.unwrap();
        let Some(wire::file_event::Event::Data(batch)) = event.event else {
            panic!("stored files only emit contents");
        };
        assert_eq!(batch.offset, contents.len() as u64);
        contents.extend_from_slice(&batch.bytes);
    }
    let completion = v1::StorageWatchFileBytesResponse::decode(invocation.await.unwrap().as_slice()).unwrap().completion.unwrap();
    assert_eq!(completed, Some(completion.clone()));
    assert_eq!(completion.bytes, 16);
    assert_eq!(contents, b"captured-content");
    fixture.finish().await;
}
