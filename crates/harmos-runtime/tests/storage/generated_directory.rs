/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// List Bounded Directory Metadata And Observe Its Initial Boundary.

/* === Tests === */

#[tokio::test]
async fn list_and_watch_existing_files() {
    use harmos_operations::storage::{ListDirectoryEntries, WatchDirectoryEntries};
    use harmos_protocol::{storage as wire, v1};
    use harmos_runtime::operations::{StreamSettings, WireStreamLimits};
    use prost::Message;
    use std::num::NonZeroUsize;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("sample.bin"), b"directory-content").unwrap();
    let fixture = super::Fixture::new(super::config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
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
    let page = fixture
        .runtime
        .operations
        .call_protobuf::<ListDirectoryEntries>(v1::StorageListDirectoryEntriesRequest {
            selection: selection.clone(),
            page: None,
            at: None,
        })
        .await
        .unwrap();
    assert!(page.next_page_token.is_empty());
    let entries = page.entries;
    assert_eq!(entries.len(), 1);
    assert!(entries[0].path.ends_with("sample.bin"));
    assert_eq!(entries[0].length, 17);
    let (invocation, streams) = fixture
        .runtime
        .operations
        .submit_protobuf::<WatchDirectoryEntries>(v1::StorageWatchDirectoryEntriesRequest { selection, include_existing: true }, limits)
        .unwrap();
    let streams = streams.unwrap();
    let first = v1::StorageWatchDirectoryEntriesResponse::decode(streams.receive_current().await.unwrap().unwrap().as_slice())
        .unwrap()
        .event
        .unwrap();
    assert!(matches!(first.event, Some(wire::directory_event::Event::Existing(entry)) if entry.path.ends_with("sample.bin")));
    let complete = v1::StorageWatchDirectoryEntriesResponse::decode(streams.receive_current().await.unwrap().unwrap().as_slice())
        .unwrap()
        .event
        .unwrap();
    assert!(matches!(complete.event, Some(wire::directory_event::Event::InitialScanComplete(_))));
    invocation.request_cancel();
    assert!(invocation.await.is_err());
    fixture.finish().await;
}
