/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Capture a Local File and Follow Its Stored Contents.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

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
async fn capture_then_follow_stored_contents() {
    use harmos_operations::storage::{CaptureFile, FollowFile};
    use harmos_protocol::storage as wire;
    use harmos_runtime::operations::{StreamSettings, WireStreamLimits};
    use prost::Message;
    use std::num::NonZeroUsize;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.bin");
    std::fs::write(&source, b"captured-content").unwrap();
    let fixture = super::Fixture::new(super::config()).await;
    fixture
        .runtime
        .operations
        .register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
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
        .submit_protobuf::<FollowFile>(
            wire::FollowFile {
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
    while let Some(bytes) = streams.receive_current().await.unwrap() {
        let event = wire::FileEvent::decode(bytes.as_slice()).unwrap();
        let Some(wire::file_event::Event::Data(batch)) = event.event else {
            panic!("stored files only emit contents");
        };
        assert_eq!(batch.offset, contents.len() as u64);
        contents.extend_from_slice(&batch.bytes);
    }
    let completion = wire::FileCompletion::decode(invocation.await.unwrap().as_slice()).unwrap();
    assert_eq!(completion.bytes, 16);
    assert_eq!(contents, b"captured-content");
    fixture.finish().await;
}
