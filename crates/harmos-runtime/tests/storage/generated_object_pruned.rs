/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Refuse An Explicitly Retired Object Watch Boundary

use super::*;
use harmos_operations::storage::{CreateSnapshot, DeleteSnapshot, WatchRows};
use harmos_protocol::{storage as wire, v1};

#[tokio::test]
async fn retired_object_watch_cut_is_refused() {
    let (runtime, storage, data) = query_fixture().await;
    let retained = runtime.operations.call_protobuf::<CreateSnapshot>(v1::StorageCreateSnapshotRequest { at: None }).await.unwrap();
    runtime.operations.call_protobuf::<DeleteSnapshot>(v1::StorageDeleteSnapshotRequest { snapshot: retained.snapshot }).await.unwrap();
    for index in 0..9 {
        storage
            .write_object(&Document {
                id: "key".into(),
                text: index.to_string(),
            })
            .await
            .unwrap();
    }
    storage.flush().await.unwrap();
    while data.objects.lock().unwrap().len() != 8 {
        data.control.pruned.notified().await;
    }
    let request = v1::StorageWatchRowsRequest {
        source: Some(wire::QuerySource {
            source: Some(wire::query_source::Source::Objects(wire::ObjectSelection {
                schema: None,
                key: None,
                versions: wire::ObjectVersions::Revisions as i32,
            })),
        }),
        at: retained.resolved_at,
    };
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut watch = runtime.operations.download_protobuf::<WatchRows>(request, limits).unwrap();
    assert!(watch.next().await.unwrap().is_none());
    assert!(watch.join().await.is_err());
    crate::support::app::shutdown(&runtime).await.unwrap();
}
