/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Foreign cursor refused.

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
async fn foreign_cursor_refused() {
    use harmos_operations::storage::FollowStream;
    let fixture = Fixture::new(config()).await;
    fixture.runtime.operations.register::<harmos_operations::Storage>(harmos_runtime::operations::Lane::default()).await.unwrap();
    let mut foreign = fixture.storage.cursor_accepted();
    foreign.history.epoch = harmos_runtime::Identifier::new("foreign").unwrap();
    let request = harmos_protocol::storage::FollowStream {
        after: Some(foreign.into()),
        selection: Some(
            harmos_runtime::storage::query::StreamSelection {
                schema: crate::storage::schema(),
                scope: None,
                item: None,
                stream: None,
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
    assert!(instance.next().await.unwrap().is_none());
    assert!(instance.join().await.is_err());
    fixture.finish().await;
}
