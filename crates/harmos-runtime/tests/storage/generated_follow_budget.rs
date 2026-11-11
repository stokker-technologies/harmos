/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Following spends budget.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

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

#[tokio::test(start_paused = true)]
async fn following_spends_budget() {
    use harmos_operations::storage::FollowRecords;
    use std::time::Duration;
    let fixture = Fixture::new(config()).await;
    fixture
        .runtime
        .operations
        .register_catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    write(&fixture.storage, "first").await;
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let request = harmos_protocol::storage::FollowRecords {
        after: None,
        selection: Some(records().into()),
    };
    let mut instance = fixture.runtime.operations.download_protobuf::<FollowRecords>(request, limits).unwrap();
    drop(instance.next().await.unwrap().unwrap());
    drop(instance.next().await.unwrap().unwrap());
    tokio::time::advance(Duration::from_secs(16)).await;
    write(&fixture.storage, "after-attempt-cap").await;
    let record = followed(instance.next().await.unwrap().unwrap());
    assert_eq!(record.item.as_str(), "after-attempt-cap");
    tokio::time::advance(Duration::from_secs(44)).await;
    assert!(instance.next().await.unwrap().is_none());
    assert!(instance.join().await.is_err());
    fixture.finish().await;
}
