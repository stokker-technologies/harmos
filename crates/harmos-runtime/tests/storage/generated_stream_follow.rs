/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Preserve Sample Sequence Independently Of Its Durable Position

use super::*;
use harmos_operations::storage::WatchRows;
use harmos_protocol::{storage as wire, v1};

#[tokio::test]
async fn sample_watch_preserves_sequence() {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, storage_catalog(), Default::default()).await.unwrap();
    let key = crate::storage::schema();
    let stream = harmos_runtime::Identifier::new("voltage").unwrap();
    let request = v1::StorageWatchRowsRequest {
        source: Some(wire::QuerySource {
            source: Some(wire::query_source::Source::Stream(
                harmos_storage::query::StreamSelection {
                    schema: key.clone(),
                    scope: None,
                    item: None,
                    stream: Some(stream.clone()),
                    source_start: None,
                    source_end: None,
                    after: 0,
                }
                .into(),
            )),
        }),
        at: None,
    };
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut watch = fixture.runtime.operations.download_protobuf::<WatchRows>(request, limits).unwrap();
    let boundary = watch.next().await.unwrap().unwrap();
    assert!(matches!(boundary.event, Some(v1::storage_watch_rows_response::Event::Boundary(_))));
    let position = fixture
        .storage
        .append_samples(vec![harmos_storage::StreamInput {
            stream,
            sequence: 17,
            record: crate::storage::record(&key, &harmos_runtime::Identifier::new("row").unwrap(), b"sample"),
        }])
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    let event = loop {
        let frame = watch.next().await.unwrap().unwrap();
        if let Some(v1::storage_watch_rows_response::Event::Sample(row)) = frame.event {
            break row;
        }
    };
    assert_eq!(event.sequence, 17);
    let row: harmos_storage::StoredRecord = event.record.unwrap().try_into().unwrap();
    assert_eq!(row.position, position);
    watch.request_cancel();
    assert!(watch.join().await.is_err());
    fixture.finish().await;
}
