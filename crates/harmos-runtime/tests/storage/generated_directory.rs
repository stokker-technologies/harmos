/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// List and Watch a Directory Through Generated Native Streams.

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
async fn list_and_watch_existing_files() {
    use harmos_operations::storage::{ListDirectory, WatchDirectory};
    use harmos_protocol::storage as wire;
    use harmos_runtime::operations::{StreamSettings, WireStreamLimits};
    use prost::Message;
    use std::num::NonZeroUsize;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("sample.bin"), b"directory-content").unwrap();
    let fixture = super::Fixture::new(super::config()).await;
    fixture.runtime.operations.register::<harmos_operations::Storage>(super::support::Lanes::Open).await.unwrap();
    let selection = Some(wire::DirectorySelection {
        root: root.path().to_str().unwrap().into(),
        recursive: false,
        rules: vec![wire::DirectoryRule { pattern: "**".into(), include: true }],
    });
    let limits = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let limits = Some(WireStreamLimits { input: limits, output: limits });
    let (invocation, streams) =
        fixture.runtime.operations.submit_protobuf::<ListDirectory>(wire::ListDirectory { selection: selection.clone() }, limits).unwrap();
    let streams = streams.unwrap();
    let mut entries = Vec::new();
    while let Some(bytes) = streams.receive_current().await.unwrap() {
        entries.push(wire::DirectoryEntry::decode(bytes.as_slice()).unwrap());
    }
    let completion = wire::DirectoryCompletion::decode(invocation.await.unwrap().as_slice()).unwrap();
    assert_eq!(completion.entries, 1);
    assert_eq!(entries.len(), 1);
    assert!(entries[0].path.ends_with("sample.bin"));
    assert_eq!(entries[0].length, 17);
    let (invocation, streams) = fixture
        .runtime
        .operations
        .submit_protobuf::<WatchDirectory>(wire::WatchDirectory { selection, include_existing: true }, limits)
        .unwrap();
    let streams = streams.unwrap();
    let first = wire::DirectoryEvent::decode(streams.receive_current().await.unwrap().unwrap().as_slice()).unwrap();
    assert!(matches!(first.event, Some(wire::directory_event::Event::Existing(entry)) if entry.path.ends_with("sample.bin")));
    let complete = wire::DirectoryEvent::decode(streams.receive_current().await.unwrap().unwrap().as_slice()).unwrap();
    assert!(matches!(complete.event, Some(wire::directory_event::Event::InitialScanComplete(_))));
    invocation.request_cancel();
    assert!(invocation.await.is_err());
    fixture.finish().await;
}
