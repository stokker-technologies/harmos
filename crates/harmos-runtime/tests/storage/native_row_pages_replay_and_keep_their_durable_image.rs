/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Replay Native Row Pages

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::super::writes;
use super::{fixture, mutation, records};
use harmos_operations::storage;
use harmos_protocol::model::PageRequest;
use harmos_query::QueryArrow;

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
async fn native_row_pages_replay_and_keep_their_durable_image() {
    let fixture = fixture().await;
    writes::seed(&fixture).await;
    let request = storage::AppendRecordsRequest {
        mutation: Some(mutation(&fixture, "rows")),
        scope: None,
        batch: Some(writes::batch(1, "native-records")),
    };
    let accepted = fixture.runtime.operations.call_native::<storage::AppendRecords>(request.clone()).await.unwrap();
    assert_eq!(fixture.runtime.operations.call_native::<storage::AppendRecords>(request).await.unwrap(), accepted);
    fixture.storage.flush().await.unwrap();
    let mut request = storage::ListRowsRequest {
        merge: None,
        source: Some(records()),
        at: None,
        transforms: Vec::new(),
        page: Some(PageRequest {
            page_size: 1,
            max_bytes: 4096,
            page_token: String::new(),
        }),
    };
    let first = fixture.runtime.operations.call_native::<storage::ListRows>(request.clone()).await.unwrap();
    request.at = first.resolved_at.clone();
    request.page.as_mut().unwrap().page_token = first.next_page_token.clone();
    assert!(!first.next_page_token.is_empty());
    let mut rows = 0;
    let mut outputs = first.rows;
    loop {
        let page = fixture.runtime.operations.call_native::<storage::ListRows>(request.clone()).await.unwrap();
        let repeated = fixture.runtime.operations.call_native::<storage::ListRows>(request.clone()).await.unwrap();
        assert_eq!(page.rows, repeated.rows);
        assert_eq!(page.next_page_token, repeated.next_page_token);
        outputs.extend(page.rows);
        if page.next_page_token.is_empty() {
            break;
        }
        request.page.as_mut().unwrap().page_token = page.next_page_token;
    }
    for output in outputs {
        if let QueryArrow::Batch(batch) = QueryArrow::try_from(output).unwrap() {
            rows += batch.as_record_batch().num_rows();
        }
    }
    assert_eq!(rows, 2);
    let mut forged = request;
    forged.page.as_mut().unwrap().page_token.push('0');
    assert!(fixture.runtime.operations.call_native::<storage::ListRows>(forged).await.is_err());
    fixture.finish().await;
}
