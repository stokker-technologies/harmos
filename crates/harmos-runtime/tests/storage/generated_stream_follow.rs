/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Stream follow sequence.

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
async fn stream_follow_sequence() {
    use harmos_operations::storage::FollowStream;
    use harmos_runtime::storage::StreamInput;
    let fixture = Fixture::new(config()).await;
    fixture.runtime.operations.register::<harmos_operations::Storage>(Lanes::Open).await.unwrap();
    let key = crate::storage::schema();
    let stream = harmos_runtime::Identifier::new("voltage").unwrap();
    let item = harmos_runtime::Identifier::new("row").unwrap();
    let request = harmos_protocol::storage::FollowStream {
        after: None,
        selection: Some(
            harmos_runtime::storage::query::StreamSelection {
                schema: key.clone(),
                scope: None,
                item: None,
                stream: Some(stream.clone()),
                source_start: None,
                source_end: None,
                after: 0,
            }
            .into(),
        ),
    };
    let limits = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let mut instance = fixture.runtime.operations.download_protobuf::<FollowStream>(request, limits).unwrap();
    fixture
        .storage
        .write_stream(vec![StreamInput {
            stream: stream.clone(),
            sequence: 17,
            record: crate::storage::record(&key, &item, b"sample"),
        }])
        .await
        .unwrap();
    let row = instance.next().await.unwrap().unwrap();
    assert_eq!(row.sequence, 17);
    assert_eq!(row.record.as_ref().unwrap().position.as_ref().unwrap().sequence, 1);
    drop(row);
    instance.request_cancel();
    assert!(instance.join().await.is_err());
    fixture.finish().await;
}
