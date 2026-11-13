/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resume Durable Records Exclusively From Their Issued Cut

use super::*;
use harmos_operations::storage::WatchRows;
use harmos_protocol::{storage as wire, v1};

#[tokio::test]
async fn record_watch_resumes() {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, storage_catalog(), Default::default()).await.unwrap();
    let request = v1::StorageWatchRowsRequest {
        source: Some(wire::QuerySource {
            source: Some(wire::query_source::Source::Records(records().into())),
        }),
        at: None,
    };
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut watch = fixture.runtime.operations.download_protobuf::<WatchRows>(request.clone(), limits).unwrap();
    let boundary = watch.next().await.unwrap().unwrap();
    assert!(matches!(boundary.event, Some(v1::storage_watch_rows_response::Event::Boundary(_))));
    let first_position = write(&fixture.storage, "first").await;
    fixture.storage.flush().await.unwrap();
    let (first, _) = watched_record(&mut watch).await;
    assert_eq!(first.position, first_position);
    assert_eq!(first.item.as_str(), "first");
    let second_position = write(&fixture.storage, "second").await;
    fixture.storage.flush().await.unwrap();
    let (second, cut) = watched_record(&mut watch).await;
    assert_eq!(second.position, second_position);
    watch.request_cancel();
    assert!(watch.join().await.is_err());
    let mut request = request;
    request.at = Some(cut);
    let mut watch = fixture.runtime.operations.download_protobuf::<WatchRows>(request, limits).unwrap();
    assert!(matches!(watch.next().await.unwrap().unwrap().event, Some(v1::storage_watch_rows_response::Event::Boundary(_))));
    let third_position = write(&fixture.storage, "reconnected").await;
    fixture.storage.flush().await.unwrap();
    let (third, _) = watched_record(&mut watch).await;
    assert_eq!(third.position, third_position);
    assert_eq!(third.item.as_str(), "reconnected");
    watch.request_cancel();
    assert!(watch.join().await.is_err());
    fixture.finish().await;
}
