/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Object follow resumes.

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

#[tokio::test]
async fn object_follow_resumes() {
    use harmos_operations::storage::FollowObjects;
    use harmos_runtime::storage::query::ObjectVersions;
    let (runtime, storage, _) = query_fixture().await;
    let first = storage.write_object(&Document { id: "key".into(), text: "saved".into() }).await.unwrap();
    storage.flush().await.unwrap();
    let mut request = harmos_protocol::storage::FollowObjects {
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
    let mut call = runtime.operations.download_protobuf::<FollowObjects>(request.clone(), limits).unwrap();
    let received: harmos_runtime::storage::ObjectEntry = call.next().await.unwrap().unwrap().try_into().unwrap();
    assert_eq!(received.version, first.version);
    drop(received);
    let second = storage.write_object(&Document { id: "key".into(), text: "live".into() }).await.unwrap();
    let received: harmos_runtime::storage::ObjectEntry = call.next().await.unwrap().unwrap().try_into().unwrap();
    assert_eq!(received.version, second.version);
    drop(received);
    storage.flush().await.unwrap();
    call.request_cancel();
    assert!(call.join().await.is_err());
    request.after = Some(second.position.into());
    let mut call = runtime.operations.download_protobuf::<FollowObjects>(request, limits).unwrap();
    let third = storage
        .write_object(&Document {
            id: "key".into(),
            text: "reconnected".into(),
        })
        .await
        .unwrap();
    let received: harmos_runtime::storage::ObjectEntry = call.next().await.unwrap().unwrap().try_into().unwrap();
    assert_eq!(received.version, third.version);
    drop(received);
    call.request_cancel();
    assert!(call.join().await.is_err());
    crate::support::app::shutdown(&runtime).await.unwrap();
}
