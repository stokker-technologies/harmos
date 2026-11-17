/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Watch Native Rows

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use super::super::writes;
use super::{fixture, mutation, records};
use harmos_operations::storage;
use harmos_runtime::operations;

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
async fn native_row_watch_delivers_durable_rows_and_settles_cancellation() {
    let fixture = fixture().await;
    writes::seed(&fixture).await;
    let settings = operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(8192).unwrap(),
    };
    let (output, input) = operations::native_stream(settings).unwrap();
    let call = fixture
        .runtime
        .operations
        .submit_native::<storage::WatchRows>(storage::WatchRowsRequest { source: Some(records()), at: None }, output)
        .unwrap();
    let boundary = input.recv().await.unwrap();
    assert!(matches!(boundary.event, Some(storage::watch_rows_response::Event::Boundary(_))));
    drop(boundary);
    fixture
        .runtime
        .operations
        .call_native::<storage::AppendRecords>(storage::AppendRecordsRequest {
            mutation: Some(mutation(&fixture, "observed")),
            scope: None,
            batch: Some(writes::batch(1, "native-records")),
        })
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    let mut items = Vec::new();
    while items.len() < 2 {
        let response = input.recv().await.unwrap();
        if let Some(storage::watch_rows_response::Event::Record(row)) = &response.event {
            items.push(row.item.to_string());
        }
    }
    assert_eq!(items, ["first", "second"]);
    call.request_cancel();
    assert!(call.await.is_err());
    fixture.finish().await;
}
