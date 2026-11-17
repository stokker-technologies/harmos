/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Upload and Retained Reads

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use super::{fixture, mutation};
use harmos_operations::storage;
use harmos_runtime::Identifier;
use harmos_runtime::storage::FileMetadata;
use harmos_runtime::storage::StagedFile;

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
async fn native_upload_resumes_fences_writers_and_preserves_read_cuts() {
    // Create And Replay One Native Production.
    let fixture = fixture().await;
    let data = b"native upload";
    let request = storage::CreateFileProductionRequest {
        metadata: Some(FileMetadata {
            id: Identifier::new("native-file").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: Some(data.len() as u64),
            expected_sha256: Some(Sha256::digest(data).into()),
        }),
        mutation: Some(mutation(&fixture, "create")),
    };
    let created = fixture.runtime.operations.call_native::<storage::CreateFileProduction>(request.clone()).await.unwrap();
    assert_eq!(fixture.runtime.operations.call_native::<storage::CreateFileProduction>(request).await.unwrap(), created);
    let producer = created.production.unwrap();
    let file = StagedFile {
        history: producer.session.history.clone(),
        id: producer.session.id.clone(),
    };
    let first = storage::AppendFileBytesRequest {
        session: Some(producer.session.clone()),
        offset: 0,
        data: data[..6].to_vec(),
        mutation: Some(mutation(&fixture, "first")),
    };
    fixture.runtime.operations.call_native::<storage::AppendFileBytes>(first.clone()).await.unwrap();
    let progress = fixture
        .runtime
        .operations
        .call_native::<storage::GetFileProduction>(storage::GetFileProductionRequest { file: Some(file.clone()), at: None })
        .await
        .unwrap();
    assert_eq!(progress.accepted_length, 6);

    // Resume Fences The Previous Writer And Keeps Its Retained Observation.
    let resumed = fixture
        .runtime
        .operations
        .call_native::<storage::CreateFileWriter>(storage::CreateFileWriterRequest {
            session: Some(producer.session.clone()),
            mutation: Some(mutation(&fixture, "resume")),
        })
        .await
        .unwrap()
        .production
        .unwrap();
    assert!(resumed.session.generation > producer.session.generation);
    let stale = storage::AppendFileBytesRequest {
        mutation: Some(mutation(&fixture, "stale")),
        offset: 6,
        data: data[6..].to_vec(),
        ..first
    };
    assert!(fixture.runtime.operations.call_native::<storage::AppendFileBytes>(stale).await.is_err());
    fixture
        .runtime
        .operations
        .call_native::<storage::AppendFileBytes>(storage::AppendFileBytesRequest {
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
        .call_native::<storage::GetFileProduction>(storage::GetFileProductionRequest {
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
        .call_native::<storage::PublishFileProduction>(storage::PublishFileProductionRequest {
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
            .call_native::<storage::GetFileBytes>(storage::GetFileBytesRequest {
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
