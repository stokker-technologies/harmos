/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private Storage Operation Journeys

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{Fixture, config, storage_catalog};
use harmos_operations::storage as operation;
use harmos_protocol::v1::PageRequest;
use harmos_runtime::{
    Identifier,
    storage::{
        FileMetadata, MutationId, StagedFile,
        query::{QueryArrow, RecordSelection, Source},
    },
};
use sha2::{Digest, Sha256};
use std::num::NonZeroUsize;

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

async fn fixture() -> Fixture {
    let fixture = Fixture::new(config()).await;
    crate::support::app::register(&fixture.runtime, storage_catalog(), harmos_runtime::operations::Lane::default()).await.unwrap();
    fixture
}
fn mutation(fixture: &Fixture, key: &str) -> MutationId {
    MutationId {
        window: fixture.storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}
fn records() -> Source {
    Source::Records(RecordSelection {
        schema: super::super::schema(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    })
}

/* === Tests === */

#[tokio::test]
async fn native_upload_resumes_fences_writers_and_preserves_read_cuts() {
    // Create And Replay One Native Production.
    let fixture = fixture().await;
    let data = b"native upload";
    let request = operation::CreateFileProductionRequest {
        metadata: Some(FileMetadata {
            id: Identifier::new("native-file").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: Some(data.len() as u64),
            expected_sha256: Some(Sha256::digest(data).into()),
        }),
        mutation: Some(mutation(&fixture, "create")),
    };
    let created = fixture.runtime.operations.call_native::<operation::CreateFileProduction>(request.clone()).await.unwrap();
    assert_eq!(fixture.runtime.operations.call_native::<operation::CreateFileProduction>(request).await.unwrap(), created);
    let producer = created.production.unwrap();
    let file = StagedFile {
        history: producer.session.history.clone(),
        id: producer.session.id.clone(),
    };
    let first = operation::AppendFileBytesRequest {
        session: Some(producer.session.clone()),
        offset: 0,
        data: data[..6].to_vec(),
        mutation: Some(mutation(&fixture, "first")),
    };
    fixture.runtime.operations.call_native::<operation::AppendFileBytes>(first.clone()).await.unwrap();
    let progress = fixture
        .runtime
        .operations
        .call_native::<operation::GetFileProduction>(operation::GetFileProductionRequest { file: Some(file.clone()), at: None })
        .await
        .unwrap();
    assert_eq!(progress.accepted_length, 6);

    // Resume Fences The Previous Writer And Keeps Its Retained Observation.
    let resumed = fixture
        .runtime
        .operations
        .call_native::<operation::CreateFileWriter>(operation::CreateFileWriterRequest {
            session: Some(producer.session.clone()),
            mutation: Some(mutation(&fixture, "resume")),
        })
        .await
        .unwrap()
        .production
        .unwrap();
    assert!(resumed.session.generation > producer.session.generation);
    let stale = operation::AppendFileBytesRequest {
        mutation: Some(mutation(&fixture, "stale")),
        offset: 6,
        data: data[6..].to_vec(),
        ..first
    };
    assert!(fixture.runtime.operations.call_native::<operation::AppendFileBytes>(stale).await.is_err());
    fixture
        .runtime
        .operations
        .call_native::<operation::AppendFileBytes>(operation::AppendFileBytesRequest {
            session: Some(resumed.session.clone()),
            mutation: Some(mutation(&fixture, "tail")),
            offset: 6,
            data: data[6..].to_vec(),
        })
        .await
        .unwrap();
    let retained = fixture
        .runtime
        .operations
        .call_native::<operation::GetFileProduction>(operation::GetFileProductionRequest {
            file: Some(file),
            at: progress.resolved_at,
        })
        .await
        .unwrap();
    assert_eq!(retained.accepted_length, 6);

    // Publish Once And Read Bounded Immutable Ranges.
    let published = fixture
        .runtime
        .operations
        .call_native::<operation::PublishFileProduction>(operation::PublishFileProductionRequest {
            session: Some(resumed.session),
            mutation: Some(mutation(&fixture, "publish")),
            length: data.len() as u64,
            sha256: Sha256::digest(data).to_vec(),
        })
        .await
        .unwrap();
    assert!(published.receipt.unwrap().completed);
    let reference = published.file.unwrap();
    let mut received = Vec::new();
    let mut at = None;
    while received.len() < data.len() {
        let response = fixture
            .runtime
            .operations
            .call_native::<operation::GetFileBytes>(operation::GetFileBytesRequest {
                file: Some(reference.clone()),
                offset: received.len() as u64,
                max_bytes: 3,
                at: at.clone(),
            })
            .await
            .unwrap();
        assert!(response.data.len() <= 3);
        assert_eq!(response.total_length, data.len() as u64);
        at = response.resolved_at;
        received.extend(response.data);
    }
    assert_eq!(received, data);
    fixture.finish().await;
}

#[tokio::test]
async fn native_row_pages_replay_and_keep_their_durable_image() {
    let fixture = fixture().await;
    super::writes::seed(&fixture).await;
    let request = operation::AppendRecordsRequest {
        mutation: Some(mutation(&fixture, "rows")),
        scope: None,
        batch: Some(super::writes::batch(1, "native-records")),
    };
    let accepted = fixture.runtime.operations.call_native::<operation::AppendRecords>(request.clone()).await.unwrap();
    assert_eq!(fixture.runtime.operations.call_native::<operation::AppendRecords>(request).await.unwrap(), accepted);
    fixture.storage.flush().await.unwrap();
    let mut request = operation::ListRowsRequest {
        source: Some(records()),
        at: None,
        transforms: Vec::new(),
        page: Some(PageRequest {
            page_size: 1,
            max_bytes: 4096,
            page_token: String::new(),
        }),
    };
    let first = fixture.runtime.operations.call_native::<operation::ListRows>(request.clone()).await.unwrap();
    request.at = first.resolved_at.clone();
    request.page.as_mut().unwrap().page_token = first.next_page_token.clone();
    assert!(!first.next_page_token.is_empty());
    let mut rows = 0;
    let mut outputs = first.rows;
    loop {
        let page = fixture.runtime.operations.call_native::<operation::ListRows>(request.clone()).await.unwrap();
        let repeated = fixture.runtime.operations.call_native::<operation::ListRows>(request.clone()).await.unwrap();
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
    assert!(fixture.runtime.operations.call_native::<operation::ListRows>(forged).await.is_err());
    fixture.finish().await;
}

#[tokio::test]
async fn native_row_watch_delivers_durable_rows_and_settles_cancellation() {
    let fixture = fixture().await;
    super::writes::seed(&fixture).await;
    let settings = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(8192).unwrap(),
    };
    let (output, input) = harmos_runtime::operations::native_stream(settings).unwrap();
    let call = fixture
        .runtime
        .operations
        .submit_native::<operation::WatchRows>(operation::WatchRowsRequest { source: Some(records()), at: None }, output)
        .unwrap();
    let boundary = input.recv().await.unwrap();
    assert!(matches!(boundary.event, Some(operation::watch_rows_response::Event::Boundary(_))));
    drop(boundary);
    fixture
        .runtime
        .operations
        .call_native::<operation::AppendRecords>(operation::AppendRecordsRequest {
            mutation: Some(mutation(&fixture, "observed")),
            scope: None,
            batch: Some(super::writes::batch(1, "native-records")),
        })
        .await
        .unwrap();
    fixture.storage.flush().await.unwrap();
    let mut items = Vec::new();
    while items.len() < 2 {
        let response = input.recv().await.unwrap();
        if let Some(operation::watch_rows_response::Event::Record(row)) = &response.event {
            items.push(row.item.to_string());
        }
    }
    assert_eq!(items, ["first", "second"]);
    call.request_cancel();
    assert!(call.await.is_err());
    fixture.finish().await;
}
