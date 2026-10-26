/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{Fixture, Lanes, config};
use harmos_runtime::Identifier;
use harmos_runtime::storage::files::FileBatch as FileBytes;
use harmos_runtime::storage::operations::{FollowProduction, ReadFile};
use harmos_runtime::storage::{FileMetadata, FilePhase, StagedFile};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn setup() -> (Fixture, harmos_runtime::storage::StorageFile, FollowProduction) {
    let fixture = Fixture::new(config()).await;
    harmos_runtime::storage::operations::register(&fixture.runtime.operations, "storage", Lanes::Open, Arc::new(fixture.storage.clone()))
        .unwrap();
    let mut producer = fixture
        .storage
        .produce_file(FileMetadata {
            id: Identifier::new("recording").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        })
        .await
        .unwrap();
    producer.write_all((b"prefix").to_vec()).await.unwrap();
    let session = producer.session().unwrap();
    let request = FollowProduction { file: StagedFile { history: session.history.clone(), id: session.id.clone() }, offset: 0 };
    (fixture, producer, request)
}

fn limits() -> harmos_runtime::operations::StreamSettings {
    harmos_runtime::operations::StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(1024).unwrap() }
}

/* === Tests === */

#[tokio::test]
async fn read_file_streams() {
    let (fixture, mut producer, _) = setup().await;
    let finished = producer.finish().await.unwrap();
    fixture.storage.flush().await.unwrap();
    let request = ReadFile {
        source: harmos_domain::model::storage::files::FileSource::Stored(finished),
        range: harmos_runtime::storage::files::ByteRange { offset: 0, limit: None },
    };
    let (sender, mut receiver) = harmos_runtime::operations::stream::<harmos_runtime::storage::files::FileBatch>(limits()).unwrap();
    let instance = fixture.runtime.operations.submit_endpoints::<ReadFile, _>(request, sender).unwrap();
    let first = receiver.recv().await.unwrap();
    assert_eq!(first.offset, 0);
    let mut bytes = first.bytes.clone();
    drop(first);

    while let Some(chunk) = receiver.recv().await {
        assert_eq!(chunk.offset, bytes.len() as u64);

        bytes.extend_from_slice(&chunk.bytes);
    }
    assert_eq!(bytes, b"prefix");
    let result = instance.await.unwrap();
    assert_eq!(result.bytes, 6);
    fixture.finish().await;
}

#[tokio::test]
async fn follow_reconnect_resumes() {
    let (fixture, mut producer, mut request) = setup().await;
    let (sender, mut receiver) = harmos_runtime::operations::stream::<FileBytes>(limits()).unwrap();
    let instance = fixture.runtime.operations.submit_endpoints::<FollowProduction, _>(request.clone(), sender).unwrap();
    let mut offset = 0;
    while offset < 6 {
        let chunk = receiver.recv().await.unwrap();
        assert_eq!(chunk.offset, offset);
        offset += chunk.bytes.len() as u64;
    }
    instance.cancel();
    assert!(instance.await.is_err());
    drop(receiver);
    producer.write_all((b"later").to_vec()).await.unwrap();
    request.offset = offset;
    let (sender, mut receiver) = harmos_runtime::operations::stream::<FileBytes>(limits()).unwrap();
    let instance = fixture.runtime.operations.submit_endpoints::<FollowProduction, _>(request, sender).unwrap();
    let mut bytes = Vec::new();
    while offset < 11 {
        let chunk = receiver.recv().await.unwrap();
        assert_eq!(chunk.offset, offset);
        bytes.extend_from_slice(&chunk.bytes);
        offset += chunk.bytes.len() as u64;
    }
    assert_eq!(bytes, b"later");
    producer.abort().await.unwrap();
    assert!(receiver.recv().await.is_none());
    let result = instance.await.unwrap();
    assert_eq!(result.length, 11);
    assert_eq!(result.phase, FilePhase::Abandoned);
    fixture.finish().await;
}

#[tokio::test]
async fn future_offset_refused() {
    let (fixture, mut producer, request) = setup().await;
    let beyond = FollowProduction { offset: 7, ..request };
    let (sender, mut receiver) = harmos_runtime::operations::stream::<FileBytes>(limits()).unwrap();
    let result = fixture.runtime.operations.submit_endpoints::<FollowProduction, _>(beyond, sender);
    if let Ok(instance) = result {
        assert!(instance.await.is_err());
    }
    assert!(receiver.recv().await.is_none());
    producer.abort().await.unwrap();
    fixture.finish().await;
}
