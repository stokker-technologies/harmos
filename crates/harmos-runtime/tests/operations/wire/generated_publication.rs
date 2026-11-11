/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Publish a Generated Native Request With Following Rows.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

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

/* === Imports === */

use harmos_operations::execution::operations::publication::{Publication, PublicationAccess, PublicationHost, Published};
use harmos_protocol::host::{FollowingRow, Publish};
use std::sync::{Arc, Mutex};

/* === Structs === */

#[derive(Default)]
struct Publications(Mutex<Vec<Publish>>);

/* === Implementations === */

impl PublicationHost for Publications {
    fn publish<'a>(&'a self, _: harmos_runtime::operations::CallContext, row: Publication<'a>) -> Published<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push(Publish {
                stream: row.stream.into(),
                sequence: row.sequence,
                published_unix_micros: row.published_unix_micros,
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
        setup.settings().operation_settings(harmos_runtime::operations::Settings { preparation_bytes: Some(std::num::NonZeroUsize::new(1024 * 1024).unwrap()), ..Default::default() });
            setup.resource::<PublicationAccess>(Arc::new(PublicationAccess(publications.clone())));
        setup.operations::<harmos_operations::Streams>(super::Lanes::Open);
    })
    .await
    .unwrap();
    let request = Publish {
        stream: "readings".into(),
        sequence: Some(4),
        published_unix_micros: Some(7),
        context: Some("batch".into()),
        payload: vec![1, 2, 3],
        following: vec![FollowingRow {
            payload: vec![4, 5],
            published_delta_micros: Some(2),
        }],
    };
    runtime.operations.call_protobuf::<harmos_operations::streams::Publish>(request.clone()).await.unwrap();
    assert_eq!(*publications.0.lock().unwrap(), vec![request]);
    runtime.shutdown().await.unwrap();
}
