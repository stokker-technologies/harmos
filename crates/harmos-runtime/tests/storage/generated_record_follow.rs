/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Record follow resumes.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn record_follow_resumes() {
    use harmos_operations::storage::FollowRecords;
    use harmos_protocol::storage::followed::Followed;
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    write(&fixture.storage, "historical").await;
    fixture.storage.flush().await.unwrap();
    let request = harmos_protocol::storage::FollowRecords {
        after: None,
        selection: Some(records().into()),
    };
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut instance = fixture.runtime.operations.download_protobuf::<FollowRecords>(request.clone(), limits).unwrap();
    let Some(Followed::Boundaries(boundaries)) = instance.next().await.unwrap().unwrap().followed else {
        panic!("a follow first reports its boundaries");
    };
    assert_eq!(boundaries.saved, boundaries.accepted);
    let first = followed(instance.next().await.unwrap().unwrap());
    assert_eq!(first.item.as_str(), "historical");
    assert!(first.position.sequence <= boundaries.saved.as_ref().unwrap().sequence);
    let second_position = write(&fixture.storage, "live").await;
    let second = followed(instance.next().await.unwrap().unwrap());
    assert_eq!(second.item.as_str(), "live");
    assert_eq!(second.position, second_position);
    assert!(second.position.sequence > boundaries.accepted.as_ref().unwrap().sequence);
    fixture.storage.flush().await.unwrap();
    instance.request_cancel();
    assert!(instance.join().await.is_err());
    let mut request = request;
    request.after = Some(second_position.into());
    let mut instance = fixture.runtime.operations.download_protobuf::<FollowRecords>(request, limits).unwrap();
    drop(instance.next().await.unwrap().unwrap());
    write(&fixture.storage, "reconnected").await;
    let third = followed(instance.next().await.unwrap().unwrap());
    assert_eq!(third.item.as_str(), "reconnected");
    instance.request_cancel();
    assert!(instance.join().await.is_err());
    fixture.finish().await;
}
