/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Follow reports pruned.

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Tests === */

#[tokio::test]
async fn follow_reports_pruned() {
    use harmos_operations::storage::FollowObjects;
    use harmos_runtime::storage::query::ObjectVersions;
    let (runtime, storage, data) = query_fixture().await;
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
    let request = harmos_protocol::storage::FollowObjects {
        after: None,
        selection: Some(
            harmos_runtime::storage::query::ObjectSelection {
                schema: None,
                key: None,
                versions: ObjectVersions::Revisions,
            }
            .into(),
        ),
    };
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut call = runtime.operations.download_protobuf::<FollowObjects>(request, limits).unwrap();
    assert!(call.next().await.unwrap().is_none());
    let error = call.join().await.err().unwrap();
    let harmos_runtime::Error::OperationFailed { source, .. } = error else {
        panic!("expected a storage operation failure");
    };
    assert!(
        matches!(source.as_ref().downcast_ref::<harmos_operations::Error>(), Some(harmos_operations::Error::Storage(harmos_storage::Error::Gap { requested, available })) if requested < available)
    );
    crate::support::app::shutdown(&runtime).await.unwrap();
}
