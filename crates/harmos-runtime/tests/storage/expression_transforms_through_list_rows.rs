/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Expressions Through Row Listing

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use arrow_array::{BooleanArray, Float64Array};

// Local Imports
use super::super::writes;
use super::{fixture, mutation, records};
use harmos_protocol::model::PageRequest;
use harmos_query::arrow::ArrowBatch;
use harmos_runtime::facade::storage::{self, Contains, Offset, RowOutput, ScalarValue, Scale, Transform};
use harmos_runtime::operations::CallContext;

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
async fn expression_transforms_through_list_rows() {
    // Persist Rows Through Native Operation Dispatch
    let fixture = fixture().await;
    writes::seed(&fixture).await;
    let call = fixture.runtime.for_call(&CallContext::default());
    let request = storage::AppendRecordsRequest {
        mutation: Some(mutation(&fixture, "expressions")),
        scope: None,
        batch: Some(writes::batch(1, "native-records")),
    };
    call.storage().append_records(request).await.unwrap();
    fixture.storage.flush().await.unwrap();

    // Compute Streaming Fields at the Saved Cut
    let transforms = vec![
        Transform::Scale(Scale {
            field: "payload.value".into(),
            name: "scaled".into(),
            factor: 2.0,
        }),
        Transform::Offset(Offset {
            field: "scaled".into(),
            name: "shifted".into(),
            amount: -1.0,
        }),
        Transform::Contains(Contains {
            field: "_storage.item".into(),
            name: "selected".into(),
            value: ScalarValue::Text("first".into()),
        }),
    ];
    let mut request = storage::ListRowsRequest {
        merge: None,
        source: Some(records()),
        at: None,
        transforms,
        page: Some(PageRequest {
            page_size: 16,
            max_bytes: 8192,
            page_token: String::new(),
        }),
    };
    let mut shifted = Vec::new();
    let mut selected = Vec::new();
    loop {
        let response = call.storage().list_rows(request.clone()).await.unwrap();
        for output in response.rows {
            if let RowOutput::Batch(bytes) = output {
                let batch = ArrowBatch::from_ipc(bytes).unwrap();
                let batch = batch.as_record_batch();
                let values = batch.column_by_name("shifted").unwrap().as_any().downcast_ref::<Float64Array>().unwrap();
                shifted.extend(values.iter());
                let values = batch.column_by_name("selected").unwrap().as_any().downcast_ref::<BooleanArray>().unwrap();
                selected.extend(values.iter());
            }
        }
        if response.next_page_token.is_empty() {
            break;
        }
        request.at = response.resolved_at;
        request.page.as_mut().unwrap().page_token = response.next_page_token;
    }
    assert_eq!(shifted, [Some(13.0), Some(21.0)]);
    assert_eq!(selected, [Some(true), Some(false)]);
    fixture.finish().await;
}
