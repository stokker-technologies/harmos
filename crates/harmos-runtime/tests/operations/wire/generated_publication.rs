/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// SamplesAppendRequest a Generated Native Request With Following Rows.

/* === Imports === */

use harmos_operations::execution::operations::publication::{Publication, PublicationAccess, PublicationHost, Published};
use harmos_protocol::host::{FollowingRow, SamplesAppendRequest};
use std::sync::{Arc, Mutex};

/* === Structs === */

#[derive(Default)]
struct Publications(Mutex<Vec<SamplesAppendRequest>>);

/* === Implementations === */

impl PublicationHost for Publications {
    fn publish<'a>(&'a self, _: harmos_runtime::operations::CallContext, row: Publication<'a>) -> Published<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push(SamplesAppendRequest {
                stream: row.stream.into(),
                sequence: row.sequence,
                published_at: row.published_unix_micros.map(|micros| prost_types::Timestamp {
                    seconds: micros.div_euclid(1_000_000),
                    nanos: (micros.rem_euclid(1_000_000) * 1000) as i32,
                }),
                context: row.context.take(),
                payload: std::mem::take(row.payload),
                following: std::mem::take(row.following),
            });
            Ok(())
        })
    }
}

/* === Tests === */

#[tokio::test]
async fn native_publication_preserves_payload_and_batch_metadata() {
    let publications = Arc::new(Publications::default());
    let runtime = harmos_runtime::Runtime::<super::app::App>::launch_with((), (), |setup| {
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(1024 * 1024).unwrap()),
            ..Default::default()
        });
        setup.resource::<PublicationAccess>(Arc::new(PublicationAccess(publications.clone())));
        setup.catalog(harmos_operations::streams_catalog(), harmos_runtime::operations::Lane::default());
    })
    .await
    .unwrap();
    let request = SamplesAppendRequest {
        stream: "readings".into(),
        sequence: Some(4),
        published_at: Some(prost_types::Timestamp { seconds: 0, nanos: 7_000 }),
        context: Some("batch".into()),
        payload: vec![1, 2, 3],
        following: vec![FollowingRow {
            payload: vec![4, 5],
            published_delta: Some(prost_types::Duration { seconds: 0, nanos: 2_000 }),
        }],
    };
    runtime.operations.call_protobuf::<harmos_operations::streams::Append>(request.clone()).await.unwrap();
    assert_eq!(*publications.0.lock().unwrap(), vec![request]);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
