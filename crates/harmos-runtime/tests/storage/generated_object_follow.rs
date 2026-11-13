/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resume Durable Object Revisions Without Replaying A Prior Revision

use super::*;
use harmos_operations::storage::WatchRows;
use harmos_protocol::{storage as wire, v1};

#[tokio::test]
async fn object_watch_resumes() {
    let (runtime, storage, _) = query_fixture().await;
    let first = storage.write_object(&Document { id: "key".into(), text: "saved".into() }).await.unwrap();
    storage.flush().await.unwrap();
    let request = v1::StorageWatchRowsRequest {
        source: Some(wire::QuerySource {
            source: Some(wire::query_source::Source::Objects(wire::ObjectSelection {
                schema: None,
                key: None,
                versions: wire::ObjectVersions::Revisions as i32,
            })),
        }),
        at: None,
    };
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut watch = runtime.operations.download_protobuf::<WatchRows>(request.clone(), limits).unwrap();
    let boundary = watch.next().await.unwrap().unwrap();
    assert_eq!(boundary.resolved_at.as_ref().unwrap().positions[0].sequence, first.position.sequence);
    let second = storage.write_object(&Document { id: "key".into(), text: "next".into() }).await.unwrap();
    storage.flush().await.unwrap();
    let (entry, cut) = loop {
        let frame = watch.next().await.unwrap().unwrap();
        if let Some(v1::storage_watch_rows_response::Event::Object(row)) = frame.event {
            let entry: harmos_storage::ObjectEntry = row.try_into().unwrap();
            break (entry, frame.resolved_at.unwrap());
        }
    };
    assert_eq!(entry.version, second.version);
    watch.request_cancel();
    assert!(watch.join().await.is_err());
    let mut request = request;
    request.at = Some(cut);
    let mut watch = runtime.operations.download_protobuf::<WatchRows>(request, limits).unwrap();
    drop(watch.next().await.unwrap().unwrap());
    let third = storage
        .write_object(&Document {
            id: "key".into(),
            text: "reconnected".into(),
        })
        .await
        .unwrap();
    storage.flush().await.unwrap();
    let received = loop {
        let frame = watch.next().await.unwrap().unwrap();
        if let Some(v1::storage_watch_rows_response::Event::Object(row)) = frame.event {
            break harmos_storage::ObjectEntry::try_from(row).unwrap();
        }
    };
    assert_eq!(received.version, third.version);
    watch.request_cancel();
    assert!(watch.join().await.is_err());
    crate::support::app::shutdown(&runtime).await.unwrap();
}
