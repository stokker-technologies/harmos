/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Apply The Declared Watch Lifetime With Controlled Time

use super::*;
use harmos_operations::storage::WatchRows;
use harmos_protocol::{storage as wire, v1};

#[tokio::test(start_paused = true)]
async fn watch_spends_its_declared_lifetime() {
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
    let mut watch = fixture.runtime.operations.download_protobuf::<WatchRows>(request, limits).unwrap();
    drop(watch.next().await.unwrap().unwrap());
    write(&fixture.storage, "first").await;
    fixture.storage.flush().await.unwrap();
    assert_eq!(watched_record(&mut watch).await.0.item.as_str(), "first");
    tokio::time::advance(std::time::Duration::from_secs(16)).await;
    write(&fixture.storage, "after-attempt-cap").await;
    fixture.storage.flush().await.unwrap();
    assert_eq!(watched_record(&mut watch).await.0.item.as_str(), "after-attempt-cap");
    tokio::time::advance(std::time::Duration::from_secs(44)).await;
    assert!(watch.next().await.unwrap().is_none());
    assert!(watch.join().await.is_err());
    fixture.finish().await;
}
