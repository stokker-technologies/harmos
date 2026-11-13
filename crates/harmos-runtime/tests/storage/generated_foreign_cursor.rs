/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Refuse A Foreign Watch Cut Before Its Opening Boundary

use super::*;
use harmos_operations::storage::WatchRows;
use harmos_protocol::{storage as wire, v1};

#[tokio::test]
async fn foreign_watch_cut_refused() {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, storage_catalog(), Default::default()).await.unwrap();
    let request = v1::StorageWatchRowsRequest {
        source: Some(wire::QuerySource {
            source: Some(wire::query_source::Source::Records(records().into())),
        }),
        at: Some(v1::ReadCut {
            positions: vec![v1::Position {
                history: Some(v1::History {
                    store: "foreign".into(),
                    epoch: "foreign".into(),
                }),
                sequence: 0,
            }],
            revisions: vec![],
        }),
    };
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut watch = fixture.runtime.operations.download_protobuf::<WatchRows>(request, limits).unwrap();
    assert!(watch.next().await.unwrap().is_none());
    assert!(watch.join().await.is_err());
    fixture.finish().await;
}
